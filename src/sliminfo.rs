/*
 *  sliminfo.rs
 * 
 *  LyMonS - worth the squeeze
 *	(c) 2020-26 Stuart Hunter
 *
 *	TODO:
 *
 *	This program is free software: you can redistribute it and/or modify
 *	it under the terms of the GNU General Public License as published by
 *	the Free Software Foundation, either version 3 of the License, or
 *	(at your option) any later version.
 *
 *	This program is distributed in the hope that it will be useful,
 *	but WITHOUT ANY WARRANTY; without even the implied warranty of
 *	MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *	GNU General Public License for more details.
 *
 *	See <http://www.gnu.org/licenses/> to get a copy of the GNU General
 *	Public License.
 *
 */

#![allow(dead_code)] // Slim/LMS player info types; some fields reserved for future display

use serde::Deserialize;
use serde_json::{json, Value};
use std::net::{UdpSocket, SocketAddrV4, Ipv4Addr, IpAddr};
use std::time::{Duration, Instant};
use std::str;
use log::{debug, info, error};
use tokio::sync::{mpsc, Mutex as TokMutex};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use std::sync::Arc;
use crate::httprpc::SlimInfoClient;

const MAX_PLAYERS: usize = 12; // Moved here as it's primarily used by LMSServer
const VARIOUS_ARTISTS: &str = "Various Artists";

use crate::deutils::{
    deserialize_bool_from_anything,
    deserialize_numeric_f64, // Used by PlayerStatus
    deserialize_numeric_i16, // Used by PlayerStatus
    default_false,    // Used by Player and PlayerStatus
};

pub fn value_to_i16(value: &Value) -> Result<i16, String> {
    match value {
        Value::Number(num) => {
            if let Some(i) = num.as_i64() {
                if i >= i16::MIN as i64 && i <= i16::MAX as i64 {
                    Ok(i as i16)
                } else {
                    Err("Value out of i16 range".to_string())
                }
            } else {
                Err("Value is not an integer".to_string())
            }
        }
        Value::Null => Err("Value is null".to_string()),
        _ => Err("Value is not a number".to_string()),
    }
}

// Player structure
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Player {
    #[serde(rename="playerindex")]
    #[serde(deserialize_with="deserialize_numeric_i16")]
    pub player_index: i16,
    #[serde(default="default_false", rename="power")]
    #[serde(deserialize_with="deserialize_bool_from_anything")]
    pub power: bool,
    #[serde(default="default_false", rename="connected")]
    #[serde(deserialize_with="deserialize_bool_from_anything")]
    pub connected: bool,
    #[serde(default="default_false", rename="isplaying")]
    #[serde(deserialize_with="deserialize_bool_from_anything")]
    pub playing: bool,
    #[serde(rename="name")]
    pub player_name: String,
    #[serde(rename="ip")]
    pub player_ip: String,
    #[serde(rename="playerid")]
    pub player_id: String,
    #[serde(rename="model")]
    pub model_type: String,
    #[serde(rename="modelname")]
    pub model_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlayerStatus {
    mode: Option<String>,
    power: Option<u8>,
    #[serde(deserialize_with="deserialize_numeric_f64")]
    time: f64,
    #[serde(rename = "mixer volume")]
    mixer_volume: Option<u8>,
    #[serde(rename = "playlist mode")]
    playlist_mode: Option<String>,
    #[serde(rename = "playlist repeat")]
    #[serde(deserialize_with="deserialize_numeric_i16")]
    playlist_repeat: i16,
    #[serde(rename = "playlist shuffle")]
    #[serde(deserialize_with="deserialize_numeric_i16")]
    playlist_shuffle: i16,
    #[serde(deserialize_with="deserialize_numeric_i16")]
    playlist_cur_index: i16,
    playlist_loop: Option<Vec<Track>>,
}

#[derive(Debug, Deserialize, Clone)]
struct Track {
    album: Option<String>,
    albumartist: Option<String>,
    artist: Option<String>,
    bitrate: Option<String>,
    compilation: Option<String>, // "0"/"1"/"N"/"Y"
    composer: Option<String>,
    conductor: Option<String>,
    performer: Option<String>,
    #[serde(deserialize_with="deserialize_numeric_f64")]
    duration: f64,  // intermittent receipt of quoted value - use deutil
    #[serde(rename = "playlist index")]
    #[serde(deserialize_with="deserialize_numeric_i16")]
    playlist_index: i16,
    remote: Option<String>,
    remotetitle: Option<String>,
    samplerate: Option<String>, // "44100" etc
    samplesize: Option<String>, // "16" etc
    title: Option<String>,
    trackartist: Option<String>,
    year: Option<String>,
    coverid: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimeField {
    pub raw: f64,
    pub display: String,
}

fn fmt_time(raw: Option<f64>) -> TimeField {
    let secs = raw.unwrap_or(0.0);
    if !secs.is_finite() || secs < 0.0 {
        return TimeField {
            raw: secs,
            display: "00:00".to_string(),
        };
    }

    let total = secs.floor() as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;

    let display = if h > 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    };

    TimeField { raw: secs, display }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlimInfo {
    pub is_playing: bool,
    pub mode: String,
    pub album: String,
    pub albumartist: String,
    pub artist: String,
    pub bitrate: String,
    pub compilation: bool,
    pub composer: String,
    pub conductor: String,
    pub duration: TimeField,
    pub tracktime: TimeField,
    pub volume: u8,
    pub remaining: TimeField,
    pub remote: bool,
    pub remotetitle: String,
    pub samplerate: i32,
    pub samplesize: i32,
    pub title: String,
    pub trackartist: String,
    pub performer: String,
    pub playlist_mode: String,
    pub repeat: u8,
    pub shuffle: u8,
    pub year: String,
    pub coverid: String,
}

impl SlimInfo {
    pub fn default() -> Self {
        SlimInfo {
            is_playing: false,
            mode: "stop".to_string(),
            album: "".to_string(),
            albumartist: "".to_string(),
            artist: "".to_string(),
            bitrate: "".to_string(),
            compilation: false,
            composer: "".to_string(),
            conductor: "".to_string(),
            duration: TimeField { raw: 0.0, display: "00:00".to_string() },
            tracktime: TimeField { raw: 0.0, display: "00:00".to_string() },
            volume: 0,
            remaining: TimeField { raw: 0.0, display: "00:00".to_string() },    
            remote: false,
            remotetitle: "".to_string(),
            samplerate: 0,
            samplesize: 0,
            title: "".to_string(),
            trackartist: "".to_string(),
            performer: "".to_string(),
            playlist_mode: "".to_string(),
            repeat: 0,
            shuffle: 0,
            year: "".to_string(),
            coverid: String::new(),
        }
    }

    pub fn from_status(ps: PlayerStatus) -> Self {
        // resolve active track index
        let cur_idx = ps.playlist_cur_index as usize;
        // pick track by playlist_cur_index; fallback to first
        let pick = |tracks: &Vec<Track>| -> Track {
            tracks
                .iter()
                .find(|t| t.playlist_index as usize == cur_idx)
                .cloned()
                .or_else(|| tracks.get(cur_idx).cloned())
                .unwrap_or_else(|| Track {
                    album: None,
                    albumartist: None,
                    artist: None,
                    bitrate: None,
                    compilation: None,
                    composer: None,
                    conductor: None,
                    duration: 0.00,
                    performer: None,
                    playlist_index: 0,
                    remote: None,
                    remotetitle: None,
                    samplerate: None,
                    samplesize: None,
                    title: None,
                    trackartist: None,
                    year: None,
                    coverid: None,
                })
        };

        let track = ps.playlist_loop.as_ref().map(pick);

        let to_bool = |s: &Option<String>| s.as_deref().map(|v| v != "0" && v != "N").unwrap_or(false);
        let parse_i32 = |s: &Option<String>| {
            s.as_deref()
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(0)
        };
        let _parse_f64 = |s: &Option<String>| {
            s.as_deref()
                .and_then(|v| v.replace('"', "").parse::<f64>().ok())
                .unwrap_or(0.0)
        };
        let s_or = |s: &Option<String>, d: &str| s.clone().unwrap_or_else(|| d.to_string());
        let u8_or = |v: Option<u8>, d: u8| v.unwrap_or(d);

        let mode = ps.mode.unwrap_or_else(|| "stop".into());
        let is_playing = ps.power.unwrap_or(0) == 1 && mode == "play";

        let d = track.as_ref().and_then(|t| Some(t.duration)).unwrap_or(0.0);
        let duration = fmt_time(Some(d));
        let tracktime = fmt_time(Some(ps.time));
        let d: f64 = duration.raw - tracktime.raw;
        let remaining = fmt_time(Some(d));
        
        let compilation = to_bool(&track.as_ref().and_then(|t| t.compilation.clone()));
        let performer = s_or(&track.as_ref().and_then(|t| t.performer.clone()), "");
        let conductor = s_or(&track.as_ref().and_then(|t| t.conductor.clone()), "");
        let mut artist = s_or(&track.as_ref().and_then(|t| t.artist.clone()), "");
        let mut albumartist = if compilation {
            VARIOUS_ARTISTS.to_string()
            
        } else {
            let mut new_value = s_or(&track.as_ref().and_then(|t| t.albumartist.clone()), "");
            if new_value.is_empty() {
                if !conductor.is_empty() {
                    new_value = conductor.clone();
                } else if !artist.is_empty() {
                    new_value = artist.clone()
                } else {
                    if !performer.is_empty() {
                        new_value = performer.clone()
                    }
                }
            }
            new_value
        };

        // and further realize the performer, conductor, artist, album artist
        let mut new_value = artist.clone();
        if albumartist.is_empty() {
            if !conductor.is_empty() {
                new_value = conductor.clone();
            }
            albumartist = new_value.clone();
        } else if albumartist != VARIOUS_ARTISTS.to_string() {
            if !artist.is_empty() {
                new_value = artist.clone();
            } else if !performer.is_empty() {
                new_value = performer.clone();
            }
            albumartist = new_value.clone();
        }
        if albumartist.is_empty() {
            if !artist.is_empty() {
                new_value = artist.clone();
            } else if !performer.is_empty() {
                new_value = performer.clone();
            }
            albumartist = new_value.clone();
        }
        if artist.is_empty() {
            if !performer.is_empty() {
                artist = performer.clone();
            }
        }

        SlimInfo {
            is_playing,
            mode,
            album: s_or(&track.as_ref().and_then(|t| t.album.clone()), ""),
            albumartist,
            artist,
            bitrate: s_or(&track.as_ref().and_then(|t| t.bitrate.clone()), ""),
            compilation,
            composer: s_or(&track.as_ref().and_then(|t| t.composer.clone()), ""),
            conductor: s_or(&track.as_ref().and_then(|t| t.conductor.clone()), ""),
            duration,
            tracktime,
            volume: u8_or(ps.mixer_volume, 0),
            remaining,
            remote: to_bool(&track.as_ref().and_then(|t| t.remote.clone())),
            remotetitle: s_or(&track.as_ref().and_then(|t| t.remotetitle.clone()), ""),
            samplerate: parse_i32(&track.as_ref().and_then(|t| t.samplerate.clone())),
            samplesize: parse_i32(&track.as_ref().and_then(|t| t.samplesize.clone())),
            title: s_or(&track.as_ref().and_then(|t| t.title.clone()), ""),
            trackartist: s_or(&track.as_ref().and_then(|t| t.trackartist.clone()), ""),
            performer,
            playlist_mode: s_or(&ps.playlist_mode, "off"),
            repeat: ps.playlist_repeat as u8,
            shuffle: ps.playlist_shuffle as u8,
            year: s_or(&track.as_ref().and_then(|t| t.year.clone()), ""),
            coverid: s_or(&track.as_ref().and_then(|t| t.coverid.clone()), ""),
        }
    }
}

// LMS structure
#[derive(Debug)]
pub struct LMSServer {
    pub player_count: i16,
    pub active_player: usize,
    pub shared_memory: String,
    pub players: Vec<Player>,
    pub refresh: bool,
    pub ready: bool,
    pub name: String,
    pub host: IpAddr,
    pub uuid: String,
    pub vers: String,
    pub port: u16,
    pub slim_tags: String,
    pub client: SlimInfoClient,
    working: bool,
    // Fields for background polling thread management
    stop_sender: Option<mpsc::Sender<()>>,
    poll_handle: Option<JoinHandle<()>>,
    // beacon watch channel
    playing_tx: watch::Sender<bool>,
    playing_rx: watch::Receiver<bool>,
    last_playing: bool,
    changed: bool,
    consecutive_poll_errors: u32,
    pub sliminfo: SlimInfo,
}

/// Number of consecutive poll failures before the connection is considered unhealthy.
/// At 200ms poll interval this is ~3 seconds of silence before we flag for reconnection.
const UNHEALTHY_THRESHOLD: u32 = 15;

impl LMSServer {
    pub fn new() -> Self {
        let (playing_tx, playing_rx) = watch::channel(false);
        LMSServer {
            player_count: -1, // no players
            active_player: usize::MAX, // no active player
            shared_memory: "".to_string(),
            players: Vec::with_capacity(MAX_PLAYERS),
            refresh: false,
            ready: false,
            name: "".to_string(),
            host: Ipv4Addr::LOCALHOST.into(),
            uuid: "".to_string(),
            vers: "".to_string(),
            port: 9000,
            slim_tags: "tags:lKeaArCckiqdxNTIzy".to_string(),
            client: SlimInfoClient::new(),
            working: false,
            stop_sender: None,
            poll_handle: None,
            playing_tx,
            playing_rx,
            last_playing: false,
            changed: false,
            consecutive_poll_errors: 0,
            sliminfo: SlimInfo::default(),
        }
    }

    pub fn subscribe_playing(&self) -> watch::Receiver<bool> {
        self.playing_rx.clone()
    }
    
    fn maybe_emit_playing(&mut self, mode: &str) {
        let is_playing = mode == "play";
        if is_playing != self.last_playing {
            self.last_playing = is_playing;
            let _ = self.playing_tx.send(is_playing); // ignore if nobody’s listening
            //debug!("Playing is now {}", if is_playing { "on" } else { "off" });
        }
    }

    pub fn reset_changed(&mut self) {
    }

    pub fn has_changed(&self) -> bool{
        self.changed
    }

    pub fn is_playing(&self) -> bool {
        self.active_player != usize::MAX && self.sliminfo.mode == "play"
    }

    /// Returns false when consecutive poll failures reach UNHEALTHY_THRESHOLD.
    /// Signals that the server or player connection has been lost.
    pub fn is_healthy(&self) -> bool {
        self.consecutive_poll_errors < UNHEALTHY_THRESHOLD
    }

    /// Returns display names of all currently known players, for diagnostic logging.
    pub fn available_player_names(&self) -> Vec<String> {
        self.players.iter()
            .map(|p| format!("\"{}\" ({})", p.player_name, p.player_id))
            .collect()
    }

    /// Sends a stop signal to the background polling task (non-blocking).
    pub fn stop_polling(&mut self) {
        if let Some(sender) = self.stop_sender.take() {
            let _ = sender.try_send(());
        }
    }

    /// Wraps this connected server in `Arc<Mutex>` and starts the background polling task.
    /// Call only after `discover()` and `get_players()` have both succeeded.
    pub async fn start_polling(mut self) -> Arc<TokMutex<LMSServer>> {
        let (tx, rx) = mpsc::channel(1);
        self.stop_sender = Some(tx);
        self.ask_refresh();

        let lms_arc = Arc::new(TokMutex::new(self));
        let lms_for_poll = Arc::clone(&lms_arc);

        let poll_handle = tokio::spawn(async move {
            let mut rx = rx;
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(200)) => {
                        let mut locked_lms = lms_for_poll.lock().await;
                        if locked_lms.refresh && locked_lms.active_player != usize::MAX {
                            match locked_lms.get_sliminfo_status().await {
                                Ok(_) => {},
                                Err(e) => error!("LMS poll error: {}", e),
                            }
                        }
                    }
                    _ = rx.recv() => {
                        debug!("LMS polling stopped.");
                        break;
                    }
                }
            }
        });

        lms_arc.lock().await.poll_handle = Some(poll_handle);
        lms_arc
    }

    /// Discovers LMS servers on the local network using UDP broadcast.
    /// Returns the first discovered `LMSServer` instance, or an error if none found within timeout.
    pub fn discover() -> Result<Self, Box<dyn std::error::Error>> {

        const LISTEN_ADDR: &str="0.0.0.0:0"; // Listen on any interface, any available port
        const BROADCAST_PORT: u16 = 3483; // Standard LMS discovery port
        const TIMEOUT_MS: u64 = 5000;
        const POLL_INTERVAL_MS: u64 = 500;
        
        let mut lms: LMSServer = LMSServer::new();

        // Create a UDP socket
        let socket = UdpSocket::bind(LISTEN_ADDR)?;
        let broadcast_addr = SocketAddrV4::new(Ipv4Addr::BROADCAST, BROADCAST_PORT);
        socket.set_broadcast(true)?;
        socket.set_nonblocking(true)?; // Crucial for polling without indefinite block

        let start_time = Instant::now();
        let timeout_duration = Duration::from_millis(TIMEOUT_MS);
        let poll_duration = Duration::from_millis(POLL_INTERVAL_MS);
        let mut buffer = [0u8; 128]; // simple string will be returned
        let payload="eJSON\0IPAD\0NAME\0VERS\0UUID\0".as_bytes();

        debug!("Attempting to discover LMS servers...");

        loop {

            if start_time.elapsed() >= timeout_duration {
                error!("Timeout: No reply received within {}ms.", TIMEOUT_MS);
                return Err("LMS server discovery timed-out".into());
            }
            // Send broadcast message
            if let Err(e) = socket.send_to(payload, broadcast_addr) {
                error!("Failed to send broadcast: {}", e);
                // Continue trying or break, depending on desired robustness
                std::thread::sleep(poll_duration);
                continue;
            }
            match socket.recv_from(&mut buffer) {
            
                Ok((num_bytes, src_addr)) => {
                    // checks here buffer not 0, not first E > 4
                    debug!("Received {} bytes from {}", num_bytes, src_addr);
                    lms.host = src_addr.ip();
                    let mut start_bytes = 5; // key + 1
                    let mut stop_bytes = 1+start_bytes + buffer[start_bytes] as usize;
                    // we should verify key is JSON
                    let port_str = str::from_utf8(&buffer[1+start_bytes..stop_bytes]).unwrap();
                    lms.port = port_str.trim_matches(|c: char| !c.is_ascii_digit()).parse::<u16>().unwrap();
                    // walk the buffer, note iterator values are for documentation purposes only
                    for _ in &["name", "vers", "uuid"] {
                        start_bytes = stop_bytes;
                        let key = str::from_utf8(&buffer[start_bytes..start_bytes+4]).unwrap();
                        start_bytes += 4;
                        stop_bytes = 1+start_bytes + buffer[start_bytes] as usize;
                        let value  = str::from_utf8(&buffer[1+start_bytes..stop_bytes]).unwrap();
                        match key {
                            "NAME" => lms.name = value.to_string(),
                            "VERS" => lms.vers = value.to_string(),
                            "UUID" => lms.uuid = value.to_string(),
                            &_ => break,
                        }
                    }

                    info!("LMS server ........: {}:{:?}", lms.host, lms.port);
                    info!("LMS name ..........: {}", lms.name);
                    info!("LMS version .......: {}", lms.vers);
                    info!("LMS UUID ..........: {}", lms.uuid);
                    lms.ready = true;
                    return Ok(lms); // Server found, parsed, and initialized
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data yet, sleep for poll interval
                    std::thread::sleep(poll_duration);
                    continue;
                }
                Err(e) => {
                    error!("Error receiving data: {}", e);
                    return Err(e.into()); // Unrecoverable error - should panic
                }

            }
        }

    }

    pub fn ask_refresh(&mut self) {
        self.refresh = true;
    }

    pub fn player_mac(&self) -> &str {
        if self.active_player != usize::MAX {
            return &self.players[self.active_player]
                .player_id.as_str();
        }
        ""
    }

    /// fetch the current status inclusive of populating tag details
    pub async fn get_sliminfo_status(&mut self) -> Result<(), Box<dyn std::error::Error>> {

        if !self.ready {
            return Err("LMS Server not discovered. Call discover() first.".into());
        }

        if self.refresh && !self.working {

            self.refresh = false;
            let command="status";
            let params = vec![json!("-"), json!("1"), json!(&self.slim_tags)]; // status inclusive supported tags

            self.working = true;
            match self.client.send_slim_request(
                self.host.to_string().as_str(),
                self.port,
                self.players[self.active_player].player_id.as_str(),
                command,
                params,
            ).await {
                Ok(result) => {
                    let status: PlayerStatus = serde_json::from_value(result)?;
                    let slim = SlimInfo::from_status(status);
                    self.changed = slim != self.sliminfo;
                    self.sliminfo = slim.clone();
                    self.maybe_emit_playing(&slim.mode.clone());
                    self.consecutive_poll_errors = 0;
                },
                Err(e) => {
                    error!("Error calling 'status' on LMS Server: {}", e);
                    self.consecutive_poll_errors += 1;
                },
            }
            self.working = false;
        }
        Ok(())
        
    }
        
    /// Fetches the player list from the LMS server and matches the configured player.
    /// Returns `Err` if the server is unreachable or the requested player is not found.
    /// Available player names are included in the error message for diagnostics.
    pub async fn get_players(&mut self, player_name_filter: &str, mac_address: &str) -> Result<(), Box<dyn std::error::Error>> {
        if !self.ready {
            return Err("LMS Server not discovered. Call discover() first.".into());
        }

        let params = vec![json!("0"), json!("99")];
        debug!("Requesting players from {}:{}...", self.host, self.port);

        let result = self.client.send_slim_request(
            self.host.to_string().as_str(),
            self.port,
            "",
            "players",
            params,
        ).await.map_err(|e| format!("LMS server unreachable while fetching players: {}", e))?;

        self.player_count = value_to_i16(&result["count"]).unwrap_or(0);
        debug!("Total players found: {}", self.player_count);

        if self.player_count > 0 {
            self.players = serde_json::from_value::<Vec<Player>>(result["players_loop"].clone())
                .map_err(|e| format!("Failed to parse player list: {}", e))?;

            for (i, player) in self.players.iter().enumerate() {
                if player_name_filter == "-" || player.player_name.to_lowercase() == player_name_filter.to_lowercase() {
                    self.active_player = i;
                    debug!("Active player: {} ({})", player.player_name, player.player_id);
                    if player.player_id.to_lowercase() == mac_address.to_lowercase() {
                        self.shared_memory = format!("/squeezelite-{}", player.player_id.to_lowercase());
                    }
                    break;
                }
            }
        }

        if self.active_player == usize::MAX {
            let available = self.players.iter()
                .map(|p| format!("\"{}\"", p.player_name))
                .collect::<Vec<_>>()
                .join(", ");
            let msg = if player_name_filter == "-" {
                "No players available on LMS server".to_string()
            } else {
                format!(
                    "Player '{}' not found — available: [{}]",
                    player_name_filter,
                    if available.is_empty() { "none".to_string() } else { available }
                )
            };
            info!("{}", msg);
            return Err(msg.into());
        }

        Ok(())
    }

    /// One-shot convenience wrapper: discover → find player → start polling.
    /// Returns `Err` if the server is not found or the player is not available.
    /// For retry-with-warning behaviour use `establish_lms_connection()` in main.
    pub async fn init_server(player_name_filter: &str, mac_address: &str) -> Result<Arc<TokMutex<LMSServer>>, Box<dyn std::error::Error>> {
        let mut lms = LMSServer::discover()?;
        lms.get_players(player_name_filter, mac_address).await?;
        Ok(lms.start_polling().await)
    }
}

impl Drop for LMSServer {
    fn drop(&mut self) {
        info!("LMSServer dropped — stopping polling task.");
        self.stop_polling();
    }
}



