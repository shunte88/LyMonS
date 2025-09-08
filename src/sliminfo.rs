/*
 *  sliminfo.rs
 * 
 *  LyMonS - worth the squeeze
 *	(c) 2020-25 Stuart Hunter
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
    deserialize_numeric_i16,
    deserialize_numeric_i32,
    deserialize_numeric_u8,
    deserialize_epoch_to_date_string,
    deserialize_seconds_to_hms,
    seconds_to_hms,   // Import seconds_to_hms for direct use in MetaTag
    default_zero_i16, // Used by PlayerStatus
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
    time: Option<f64>,
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
    duration: Option<f64>,
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
                    duration: None,
                    performer: None,
                    playlist_index: 0,
                    remote: None,
                    remotetitle: None,
                    samplerate: None,
                    samplesize: None,
                    title: None,
                    trackartist: None,
                    year: None,
                })
        };

        let track = ps.playlist_loop.as_ref().map(pick);

        let to_bool = |s: &Option<String>| s.as_deref().map(|v| v != "0" && v != "N").unwrap_or(false);
        let parse_i32 = |s: &Option<String>| {
            s.as_deref()
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(0)
        };
        let s_or = |s: &Option<String>, d: &str| s.clone().unwrap_or_else(|| d.to_string());
        let _f_or = |f: Option<f64>, d: f64| f.unwrap_or(d);
        let u8_or = |v: Option<u8>, d: u8| v.unwrap_or(d);

        let mode = ps.mode.unwrap_or_else(|| "stop".into());
        let is_playing = ps.power.unwrap_or(0) == 1 && mode == "play";

        let duration = fmt_time(track.as_ref().and_then(|t| t.duration));
        let tracktime = fmt_time(ps.time);
        let dur: f64 = duration.raw - tracktime.raw;
        let remaining = fmt_time(Some(dur));
        
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
    pub sliminfo: SlimInfo,
}

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
        //self.players[self.active_player].playing
        // the boolean on the play is likely better but mode has the detail too
        if self.active_player != usize::MAX && self.sliminfo.mode == "play" {
            true
        }else {
            false
        }
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
                    //println!("{:?}", result);
                    let status: PlayerStatus = serde_json::from_value(result)?;
                    let slim = SlimInfo::from_status(status);
                    self.changed = slim != self.sliminfo;
                    self.sliminfo = slim.clone();
                    self.maybe_emit_playing(&slim.mode.clone());
                },
                Err(e) => error!("Error calling 'status' on LMS Server: {}", e),
            }
            self.working = false;
        }
        Ok(())
        
    }
        
    /// Fetches player information from the discovered LMS server.
    /// This method assumes `self.host` and `self.port` are already populated by `discover()`.
    pub async fn get_players(&mut self, player_name_filter: &str, mac_address: &str) -> Result<(), Box<dyn std::error::Error>> {
        if !self.ready {
            return Err("LMS Server not discovered. Call discover() first.".into());
        }
        
        let command="players";
        let params = vec![json!("0"), json!("99")]; // Requesting players from index 0 to 99
        
        debug!("Requesting players from {}:{}...", self.host, self.port);
        match self.client.send_slim_request(
            self.host.to_string().as_str(),
            self.port,
            "", // Player MAC is empty for this command
            command,
            params,
        ).await {
            Ok(result) => {
                self.player_count = value_to_i16(&result["count"]).unwrap_or(0);
                debug!("Total players found: {}", self.player_count);
                
                if self.player_count > 0 {
                    self.players = serde_json::from_value::<Vec<Player>>(result["players_loop"].clone())
                    .expect(&format!("{} json parser", command));
                
                    // Iterate through the players to find the active player
                    for (i, player) in self.players.iter().enumerate() {
                        if player_name_filter == "-" || player.player_name.to_lowercase() == player_name_filter.to_lowercase() {
                            self.active_player = i; // Store the index
                            debug!("Active player set to: {} ({})", player.player_name, player.player_id);
                            if player.player_id.to_lowercase() == mac_address.to_lowercase() {
                                self.shared_memory = format!("/squeezelite-{}", player.player_id.clone().to_lowercase());
                            }
                            break; // Found the player, no need to continue
                        }
                    }
                    
                    if self.active_player == usize::MAX && player_name_filter != "-" {
                        debug!("Player '{}' not found among discovered players.", player_name_filter);
                    } else if self.active_player == usize::MAX {
                        info!("No specific player requested or no players found.");
                    }
                } else {
                    info!("No players reported by LMS server.");
                }
            },
            Err(e) => error!("Error calling 'players' on LMS Server: {}", e),
        }
        Ok(())
    }

    /// Initializes the LMS server, discovers it, fetches players, initializes tags,
    /// and starts a background polling thread for status updates.
    ///
    /// Returns the LMSServer instance wrapped in an `Arc<TokMutex>` on success,
    /// or an error if initialization fails.
    pub async fn init_server(player_name_filter: &str, mac_address: &str) -> Result<Arc<TokMutex<LMSServer>>, Box<dyn std::error::Error>> {
        // Directly initialize lms with the result of discover() to avoid unused_assignments warning
        let mut lms = LMSServer::discover()?;
        
        if lms.ready {
            lms.get_players(player_name_filter, mac_address).await?;
            if lms.player_count > 0 && lms.active_player != usize::MAX { // Ensure an active player is found
                lms.ask_refresh();

                // Create a channel for stopping the polling thread
                let (tx, rx) = mpsc::channel(1);
                lms.stop_sender = Some(tx);

                // Wrap the LMSServer instance in Arc<TokMutex> for shared mutable access
                let lms_arc = Arc::new(TokMutex::new(lms));
                let lms_for_poll = Arc::clone(&lms_arc);

                // Spawn the background polling task
                let poll_handle = tokio::spawn(async move {
                    let mut rx = rx; // Move receiver into the async block
                    loop {
                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_millis(200)) => { // Poll every 1/5 second
                                let mut locked_lms = lms_for_poll.lock().await;
                                if locked_lms.refresh {
                                    if locked_lms.active_player != usize::MAX {
                                        let _player_id = locked_lms.players[locked_lms.active_player].player_id.clone();
                                        match locked_lms.get_sliminfo_status().await {
                                            Ok(_) => {},
                                            Err(e) => error!("Error updating LMS status in polling thread: {}", e),
                                        }
                                    } else {
                                        debug!("No active player selected for status polling.");
                                    }
                                }
                            }
                            _ = rx.recv() => {
                                debug!("LMS polling thread received stop signal. Exiting.");
                                break; // Exit the loop and terminate the thread
                            }
                        }
                    }
                });
                // Store the JoinHandle in the original LMSServer instance (which is now part of lms_arc)
                lms_arc.lock().await.poll_handle = Some(poll_handle);
                
                Ok(lms_arc) // Return the Arc<TokMutex<LMSServer>>
            } else {
                // If no active player, no polling thread is started.
                // We still need to return an Arc<TokMutex<LMSServer>> to match the signature.
                // This means we wrap the initial `lms` in an Arc<TokMutex> and return it.
                info!("No active LMS players found. Returning initial LMSServer instance.");
                Ok(Arc::new(TokMutex::new(lms)))
            }
        } else {
            info!("LMS Server not found during discovery. Returning initial LMSServer instance.");
            // If discovery failed, return the initial `lms` wrapped in Arc<TokMutex>
            Ok(Arc::new(TokMutex::new(lms)))
        }
    }
}

// Implement Drop trait to stop the background thread when LMSServer goes out of scope
impl Drop for LMSServer {
    fn drop(&mut self) {
        info!("LMSServer dropped. Attempting to stop polling thread...");
        if let Some(sender) = self.stop_sender.take() {
            // Send a stop signal. This is non-blocking.
            if let Err(e) = sender.try_send(()) {
                error!("Failed to send stop signal to polling thread: {}", e);
            }
        }
        // Note: Joining the thread here (self.poll_handle.take().unwrap().await)
        // would require this Drop impl to be async or to block on a runtime,
        // which is generally discouraged in Drop implementations.
        // For graceful shutdown, a dedicated async `shutdown` method is usually preferred.
        // Here, we just send the signal and let the runtime clean up the detached task.
    }
}



