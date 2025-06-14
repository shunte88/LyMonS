use serde_json::{json, Value};
use serde::{Deserialize};
use std::net::{UdpSocket, SocketAddrV4, Ipv4Addr, IpAddr};
use std::time::{Duration, Instant};
use std::str;
use log::{debug, info, error};
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use std::sync::Arc;

// Import necessary items from deutils
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
use crate::httprpc::SlimInfoClient;

const MAX_PLAYERS: usize = 12; // Moved here as it's primarily used by LMSServer

// Helper function that was in main.rs, now moved here as it's used by LMSServer
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

// Helper function that was in main.rs, now moved here as it's used by LMSServer
pub fn flatten_json(json: &Value, prefix: &str) -> serde_json::Map<String, Value> {
    let mut result: serde_json::Map<String, Value> = serde_json::Map::new();
    match json {
        Value::Object(object) => {
            for (key, value) in object {
                let new_key = if prefix.is_empty() {
                    key.to_string()
                } else {
                    format!("{}.{}", prefix, key)
                };
                result.extend(flatten_json(value, &new_key));
            }
        }
        Value::Array(array) => {
            for (index, value) in array.iter().enumerate() {
                let new_key = if prefix.is_empty() {
                    format!("{}", index)
                } else {
                    format!("{}.{}", prefix, index)
                };
                result.extend(flatten_json(value, &new_key));
            }
        }
        _ => {
            result.insert(prefix.to_string(), json.clone());
        }
    }
    result
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

/*
    model       modelname
    =========== ==============
    slimp3      SliMP3
    Squeezebox  Squeezebox 1
    squeezebox2 Squeezebox 2
    squeezebox3 Squeezebox 3
    transporter Transporter
    receiver    Squeezebox Receiver
    boom        Squeezebox Boom
    softsqueeze Softsqueeze
    controller  Squeezebox Controller
    squeezeplay SqueezePlay
    squeezelite SqueezeLite
    baby        Squeezebox Radio
    fab4        Squeezebox Touch
*/

// Tag structure
#[derive(Debug, Clone)]
pub struct MetaTag {
    pub name: String,
    flat_name: String,
    lmstag: String,
    pub raw_value: String, // value
    pub value: String,     // display_value
    method: String,
    pub valid: bool,
    pub changed: bool,
}

impl Default for MetaTag {
    fn default() -> Self {
        MetaTag {
            name: "".to_string(),
            flat_name: "".to_string(),
            lmstag: "".to_string(),
            raw_value: "".to_string(),
            value: "".to_string(),
            method: "*".to_string(),
            valid: false,
            changed: false,
        }
    }
}

impl MetaTag {
    // New special method to apply transformations based on `self.method`
    fn special_method(&self, value: &str) -> String {
        if self.method.as_str() == "string_to_hms" && !value.is_empty() {
            if let Ok(parsed_value) = value.parse::<f32>() {
                seconds_to_hms(parsed_value) // Call the imported seconds_to_hms
            } else {
                value.to_string() // Return original value if parsing fails
            }
        }
        else {
            value.to_string() // Return original value if no special method or method doesn't match
        }
    }

    // rethink this so we have value and a formatted display_value
    pub fn set_value(&mut self, new_value: &str) {
        // capture state value 
        self.raw_value = new_value.to_string();
        // Apply special parse methods prior to conditional test and modify
        let my_value = self.special_method(new_value);
        if self.value.to_ascii_lowercase() != my_value.to_ascii_lowercase() {
            self.value = my_value; // my_value is already a String
            self.changed = true;
            self.valid = true;
            info!("{} :: {}", self.name.replace('_', " ").to_ascii_lowercase(), self.value);
        } else {
            self.changed = false;
        
        }
    }

}

/// Track information
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Track {
    #[serde(default, deserialize_with="deserialize_numeric_i32")]
    pub album_id: i32,
    #[serde(default, deserialize_with="deserialize_numeric_i32", rename="id")]
    pub track_id: i32,
    #[serde(default, rename="coverid")]
    pub cover_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub album: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default, deserialize_with="deserialize_seconds_to_hms")] // Apply deserializer
    pub duration: String, // Changed type to String to hold formatted duration
    #[serde(default, deserialize_with="deserialize_seconds_to_hms")] // Apply deserializer
    pub remaining: String, // Changed type to String to hold formatted duration
    #[serde(default)]
    pub albumartist: String,
    #[serde(default)]
    pub artistrole: String,
    #[serde(default)]
    pub bitrate: String,
    #[serde(default, deserialize_with="deserialize_bool_from_anything")]
    pub compilation: bool,
    #[serde(default)]
    pub composer: String,
    #[serde(default)]
    pub conductor: String,
    #[serde(default, deserialize_with="deserialize_numeric_u8")]
    pub disc: u8,
    #[serde(default, deserialize_with="deserialize_numeric_u8")]
    pub disccount: u8,
    #[serde(default)]
    pub performer: String,
    #[serde(default)]
    pub remotetitle: String,
    #[serde(default)]
    pub trackartist: String,
    #[serde(default, deserialize_with="deserialize_numeric_u8")]
    pub tracknum: u8,
    #[serde(default)]
    pub year: String,
    #[serde(default, rename="playlist index")]
    pub playlist_index: i32,
}

/// Player status and current playing track
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct PlayerStatus {
    #[serde(default)]
    pub mode: String,
    #[serde(default="default_zero_i16", rename="playlist repeat")]
    pub playlist_repeat: i16,
    #[serde(default="default_zero_i16", rename="playlist shuffle")]
    pub playlist_shuffle: i16,
    #[serde(deserialize_with="deserialize_bool_from_anything")]
    pub power: bool,
    #[serde(default="default_zero_i16", rename="mixer volume")] // a percent
    pub volume: i16,
    #[serde(default, deserialize_with="deserialize_seconds_to_hms")]
    pub duration: String,
    #[serde(default, deserialize_with="deserialize_epoch_to_date_string")]
    pub date: String,
    #[serde(default, deserialize_with="deserialize_seconds_to_hms")]
    pub time: String,
    #[serde(default="default_zero_i16")]
    pub can_seek: i16,
    #[serde(default)]
    pub playlist_loop: Vec<Track>,
}

// Tag types enum
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(usize)]
pub enum TagID {
    ALBUM = 0,
    ALBUMARTIST,
    ALBUMID,
    ARTIST,
    ARTISTROLE,
    BITRATE,
    COMPILATION,
    COMPOSER,
    CONDUCTOR,
    CONNECTED,
    DISC,
    DISCCOUNT,
    DURATION,
    MODE,
    PERFORMER,
    POWER,
    REMAINING,
    REMOTE,
    REMOTETITLE,
    REPEAT,
    SAMPLERATE,
    SAMPLESIZE,
    SERVER, // not a real tag - just using infrastructure
    SHUFFLE,
    TIME,
    TITLE,
    TRACKARTIST,
    TRACKCOUNT,
    TRACKID, // unique ID!!!
    TRACKNUM,
    VOLUME,
    YEAR,
    MAXTAG,
}

const MAXTAG_TYPES: usize = TagID::MAXTAG as usize;

// LMS structure
#[derive(Debug)]
pub struct LMSServer {
    pub player_count: i16,
    pub active_player: usize,
    pub players: Vec<Player>,
    pub refresh: bool,
    pub ready: bool,
    pub name: String,
    pub host: IpAddr,
    pub uuid: String,
    pub vers: String,
    pub port: u16,
    pub slim_tags: String,
    pub tags: Vec<MetaTag>,
    pub client: SlimInfoClient,
    // Fields for background polling thread management
    stop_sender: Option<mpsc::Sender<()>>,
    poll_handle: Option<JoinHandle<()>>,
}

impl LMSServer {
    pub fn new() -> Self {
        LMSServer {
            player_count: -1, // no players
            active_player: usize::MAX, // no active player
            players: Vec::with_capacity(MAX_PLAYERS),
            refresh: false,
            ready: false,
            name: "".to_string(),
            host: Ipv4Addr::LOCALHOST.into(),
            uuid: "".to_string(),
            vers: "".to_string(),
            port: 9000,
            slim_tags: "tags:".to_string(),
            tags: Vec::with_capacity(MAXTAG_TYPES),
            client: SlimInfoClient::new(),
            stop_sender: None,
            poll_handle: None,
        }
    }

    pub fn reset_changed(&mut self) {
        for ti in 0..MAXTAG_TYPES {
            self.tags[ti].changed = false;
        }
    }

    pub fn has_changed(&self) -> bool{
        for ti in 0..MAXTAG_TYPES {
            if self.tags[ti].changed {
                return true;
            }
        }
        false
    }

    pub fn is_playing(&self) -> bool {
        //self.players[self.active_player].playing
        // the boolean on the play is likely better but mode has the detail too
        if self.active_player != usize::MAX && self.tags[TagID::MODE as usize].valid && self.tags[TagID::MODE as usize].value == "play" {
            true
        }else {
            false
        }
    }

    pub fn populate_tag(&mut self, tagidx: usize, name: &str, lmstag: &str, also_flat: bool, method: &str) {
        if tagidx < MAXTAG_TYPES {
            self.tags[tagidx].name = name.to_string();
            if also_flat {
                self.tags[tagidx].flat_name = format!("playlist_loop.0.{}",name);
            }else {
                self.tags[tagidx].flat_name = "".to_string();
            
            }
            self.tags[tagidx].name = name.to_string();
            self.tags[tagidx].lmstag = lmstag.to_string();
            self.tags[tagidx].method = method.to_string();
            self.tags[tagidx].valid = false;
            self.tags[tagidx].changed = false;
        }
    }

    pub fn init_tags(&mut self) {
        self.tags.resize(MAXTAG_TYPES, MetaTag::default());
        self.populate_tag(TagID::ALBUM as usize, "album", "l", true, "*");
        self.populate_tag(TagID::ALBUMARTIST as usize, "albumartist", "K", true, "*");
        self.populate_tag(TagID::ALBUMID as usize, "album_id", "e",true, "*");
        self.populate_tag(TagID::ARTIST as usize, "artist", "a",true, "*");
        self.populate_tag(TagID::ARTISTROLE as usize, "artistrole", "A", true, "*");
        self.populate_tag(TagID::BITRATE as usize, "bitrate", "r",true, "*");
        self.populate_tag(TagID::COMPILATION as usize, "compilation", "C",true, "*");
        self.populate_tag(TagID::COMPOSER as usize, "composer", "c",true, "*");
        self.populate_tag(TagID::CONNECTED as usize, "player_connected", "k",false, "*");
        self.populate_tag(TagID::CONDUCTOR as usize, "conductor", "",true, "*");
        self.populate_tag(TagID::DISC as usize, "disc", "i",true, "*");
        self.populate_tag(TagID::DISCCOUNT as usize, "disccount", "q",true, "*");
        self.populate_tag(TagID::DURATION as usize, "duration", "d",false, "string_to_hms");
        self.populate_tag(TagID::MODE as usize, "mode", "",false, "*");
        self.populate_tag(TagID::PERFORMER as usize, "performer", "",true, "*");
        self.populate_tag(TagID::POWER as usize, "power", "",false, "*");
        self.populate_tag(TagID::REMAINING as usize, "remaining", "",false, "string_to_hms");
        self.populate_tag(TagID::REMOTE as usize, "remote", "x",true, "*");
        self.populate_tag(TagID::REMOTETITLE as usize, "remote_title", "N",true, "*");
        self.populate_tag(TagID::REPEAT as usize, "playlist repeat", "",false, "*");
        self.populate_tag(TagID::SAMPLERATE as usize, "samplerate", "T",true, "*");
        self.populate_tag(TagID::SAMPLESIZE as usize, "samplesize", "I",true, "*");
        // pseudo tag - used to model server bounce
        self.populate_tag(TagID::SERVER as usize, "not_real_server_not_real", "",false, "*");
        self.populate_tag(TagID::SHUFFLE as usize, "playlist shuffle", "",false, "*");
        self.populate_tag(TagID::TIME as usize, "time", "",true, "string_to_hms");
        self.populate_tag(TagID::TITLE as usize, "title", "",true, "*");
        self.populate_tag(TagID::TRACKARTIST as usize, "trackartist", "",true, "*");
        self.populate_tag(TagID::TRACKID as usize, "id", "",true, "*");
        self.populate_tag(TagID::TRACKCOUNT as usize, "tracks", "z",true, "*");
        self.populate_tag(TagID::TRACKNUM as usize, "tracknum", "",true, "*");
        self.populate_tag(TagID::VOLUME as usize, "mixer volume", "",false, "*");
        self.populate_tag(TagID::YEAR as usize, "year", "y",true, "*");

        // construct a tag request string and set for sliminfo calls
        for ti in 0..MAXTAG_TYPES {
            self.slim_tags.push_str(&self.tags[ti].lmstag);
        }

    }

    /// Discovers LMS servers on the local network using UDP broadcast.
    /// Returns the first discovered `LMSServer` instance, or an error if none found within timeout.
    pub fn discover() -> Result<Self, Box<dyn std::error::Error>> {

        const LISTEN_ADDR: &str="0.0.0.0:0"; // Listen on any interface, any available port
        const BROADCAST_PORT: u16 = 3483; // Standard LMS discovery port
        const TIMEOUT_MS: u64 = 5000;
        const POLL_INTERVAL_MS: u64 = 250;
        
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

        info!("Attempting to discover LMS servers...");

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

    /// fetch the current status inclusive of populating tag details
    pub async fn get_sliminfo_status(&mut self) -> Result<(), Box<dyn std::error::Error>> {

        const VARIOUS_ARTISTS: &str = "Various Artists";

        if !self.ready {
            return Err("LMS Server not discovered. Call discover() first.".into());
        }

        if self.refresh {

            self.refresh = false;
            let command="status";
            let params = vec![json!("-"), json!("1"), json!(&self.slim_tags)]; // status inclusive supported tags

            let mut dtime:f32 = 0.0;
            let mut ptime = dtime;

            //info!("Requesting status for player {} from {}:{}...",
            //  self.players[self.active_player].player_name, self.host, self.port);

            match self.client.send_slim_request(
                self.host.to_string().as_str(),
                self.port,
                self.players[self.active_player].player_id.as_str(),
                command,
                params,
            ).await {
                Ok(result) => {
                    let flattened = flatten_json(&result, "");
                    for ti in 0..MAXTAG_TYPES {
                        // Check if the "name" tag exists in the direct result
                        if let Some(this) = result.get(&self.tags[ti].name.clone()) {
                            let tag_value = if let Some(s) = this.as_str() {
                                s.to_string()
                            } else {
                                this.to_string()
                            };
                            self.tags[ti].set_value(&tag_value);
                            if ti == TagID::DURATION as usize {
                                if let Ok(parsed_value) = tag_value.parse::<f32>() {
                                    dtime = parsed_value;
                                }
                            } else if ti == TagID::TIME as usize {
                                if let Ok(parsed_value) = tag_value.parse::<f32>() {
                                    ptime = parsed_value;
                                }
                            }
                        }
                        // If not set and a flat_name is defined, check the flattened map
                        if !self.tags[ti].changed && !self.tags[ti].flat_name.is_empty() {
                            if let Some(this) = flattened.get(&self.tags[ti].flat_name.clone()) {
                                let tag_value = if let Some(s) = this.as_str() {
                                    s.to_string()
                                } else {
                                    this.to_string()
                                };
                                self.tags[ti].set_value(&tag_value);
                            }
                        }
                    }
                    if self.has_changed() {

                        // fix remaining
                        self.tags[TagID::REMAINING as usize].set_value(format!("{:.4}", (dtime - ptime)).as_str());

                        // fix Various Artists when Compilation
                        if !self.tags[TagID::COMPILATION as usize].value.is_empty() && self.tags[TagID::COMPILATION as usize].value  == "1" {
                            self.tags[TagID::ALBUMARTIST as usize].set_value(VARIOUS_ARTISTS);
                        }

                        // and rationalize the performer, conductor, artist, album artist
                        let mut new_value = self.tags[TagID::ARTIST as usize].value.clone();
                        if self.tags[TagID::ALBUMARTIST as usize].value.is_empty() {
                            if !self.tags[TagID::CONDUCTOR as usize].value.is_empty() {
                                new_value = self.tags[TagID::CONDUCTOR as usize].value.clone();
                            }
                            self.tags[TagID::ALBUMARTIST as usize].set_value(&new_value.as_str());
                        } else if self.tags[TagID::ALBUMARTIST as usize].value != VARIOUS_ARTISTS {
                            if !self.tags[TagID::ARTIST as usize].value.is_empty() {
                                new_value = self.tags[TagID::ARTIST as usize].value.clone();
                            } else if !self.tags[TagID::PERFORMER as usize].value.is_empty() {
                                new_value = self.tags[TagID::PERFORMER as usize].value.clone();
                            }
                            self.tags[TagID::ALBUMARTIST as usize].set_value(&new_value.as_str());
                        }
                        if self.tags[TagID::ALBUMARTIST as usize].value.is_empty() {
                            if !self.tags[TagID::ARTIST as usize].value.is_empty() {
                                new_value = self.tags[TagID::ARTIST as usize].value.clone();
                            } else if !self.tags[TagID::PERFORMER as usize].value.is_empty() {
                                new_value = self.tags[TagID::PERFORMER as usize].value.clone();
                            }
                            self.tags[TagID::ALBUMARTIST as usize].set_value(&new_value.as_str());
                        }
                        if self.tags[TagID::ARTIST as usize].value.is_empty() {
                            if !self.tags[TagID::PERFORMER as usize].value.is_empty() {
                                new_value = self.tags[TagID::PERFORMER as usize].value.clone();
                                self.tags[TagID::ARTIST as usize].set_value(&new_value.as_str());
                            }
                        }
                    }
                },
                Err(e) => error!("Error calling 'status' on LMS Server: {}", e),
            }
        }
        Ok(())
        
    }
        
    /// Fetches player information from the discovered LMS server.
    /// This method assumes `self.host` and `self.port` are already populated by `discover()`.
    pub async fn get_players(&mut self, player_name_filter: &str) -> Result<(), Box<dyn std::error::Error>> {
        if !self.ready {
            return Err("LMS Server not discovered. Call discover() first.".into());
        }
        
        let command="players";
        let params = vec![json!("0"), json!("99")]; // Requesting players from index 0 to 99
        
        info!("Requesting players from {}:{}...", self.host, self.port);
        match self.client.send_slim_request(
            self.host.to_string().as_str(),
            self.port,
            "", // Player MAC is empty for this command
            command,
            params,
        ).await {
            Ok(result) => {
                self.player_count = value_to_i16(&result["count"]).unwrap_or(0);
                info!("Total players found: {}", self.player_count);
                
                if self.player_count > 0 {
                    self.players = serde_json::from_value::<Vec<Player>>(result["players_loop"].clone())
                    .expect(&format!("{} json parser", command));
                
                // Iterate through the players to find the active player
                for (i, player) in self.players.iter().enumerate() {
                    if player_name_filter == "-" || player.player_name.to_lowercase() == player_name_filter.to_lowercase() {
                        self.active_player = i; // Store the index
                            info!("Active player set to: {} ({})", player.player_name, player.player_id);
                            break; // Found the player, no need to continue
                        }
                    }
                    
                    if self.active_player == usize::MAX && player_name_filter != "-" {
                        info!("Player '{}' not found among discovered players.", player_name_filter);
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
    /// Returns the LMSServer instance wrapped in an `Arc<Mutex>` on success,
    /// or an error if initialization fails.
    pub async fn init_server(player_name_filter: &str) -> Result<Arc<Mutex<LMSServer>>, Box<dyn std::error::Error>> {
        // Directly initialize lms with the result of discover() to avoid unused_assignments warning
        let mut lms = LMSServer::discover()?;
        
        if lms.ready {
            lms.get_players(player_name_filter).await?;
            if lms.player_count > 0 && lms.active_player != usize::MAX { // Ensure an active player is found
                lms.init_tags();
                lms.ask_refresh();

                // Create a channel for stopping the polling thread
                let (tx, rx) = mpsc::channel(1);
                lms.stop_sender = Some(tx);

                // Wrap the LMSServer instance in Arc<Mutex> for shared mutable access
                let lms_arc = Arc::new(Mutex::new(lms));
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
                                            Ok(_) => debug!("LMS status updated successfully."),
                                            Err(e) => error!("Error updating LMS status in polling thread: {}", e),
                                        }
                                    } else {
                                        debug!("No active player selected for status polling.");
                                    }
                                }
                            }
                            _ = rx.recv() => {
                                info!("LMS polling thread received stop signal. Exiting.");
                                break; // Exit the loop and terminate the thread
                            }
                        }
                    }
                });
                // Store the JoinHandle in the original LMSServer instance (which is now part of lms_arc)
                lms_arc.lock().await.poll_handle = Some(poll_handle);
                
                Ok(lms_arc) // Return the Arc<Mutex<LMSServer>>
            } else {
                // If no active player, no polling thread is started.
                // We still need to return an Arc<Mutex<LMSServer>> to match the signature.
                // This means we wrap the initial `lms` in an Arc<Mutex> and return it.
                info!("No active LMS players found. Returning initial LMSServer instance.");
                Ok(Arc::new(Mutex::new(lms)))
            }
        } else {
            info!("LMS Server not found during discovery. Returning initial LMSServer instance.");
            // If discovery failed, return the initial `lms` wrapped in Arc<Mutex>
            Ok(Arc::new(Mutex::new(lms)))
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
