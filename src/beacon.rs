// src/beacon.rs  (async / tokio mpsc)

use tokio::sync::mpsc::{self, Sender, Receiver};
use tokio::time::{self, Duration, Instant};
use log::{debug, warn};
use serde_json::json;
use std::io;

#[derive(Clone, Debug)]
pub struct LmsConfig {
    pub server: String,   // "192.168.1.23"
    pub port:   u16,      // 9000
    pub mac:    String,   // "dc:a6:32:61:88:58"
    /// Poll cadence. Suggested: 250ms
    pub poll:   Duration,
}

impl LmsConfig {
    pub fn new(server: &str, port: u16, mac: &str, poll: Duration) -> Self {
        LmsConfig {
            server: server.to_string(),
            port,
            mac: mac.to_string(),
            poll,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlayingEvent {
    pub playing: bool,    // true when mode == "play"
    pub mode:    String,  // "play", "pause", "stop", or other/empty on error
    pub ts:      Instant,
}

#[derive(Debug)]
pub enum StatusCmd {
    Shutdown,
}

/// Spawn an async beacon that polls LMS `status` and sends `PlayingEvent`s on change.
/// Returns (cmd_tx, event_rx).
pub fn spawn_status_beacon(cfg: LmsConfig) -> io::Result<(Sender<StatusCmd>, Receiver<PlayingEvent>)> {
    // small, bounded channels
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<StatusCmd>(8);
    let (evt_tx, evt_rx) = mpsc::channel::<PlayingEvent>(32);

    // Build client up-front so we can return an error if it fails
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(800))
        .build()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("reqwest build: {e}")))?;

    let url = format!("http://{}:{}/jsonrpc.js", cfg.server, cfg.port);
    let poll = cfg.poll;

    tokio::spawn(async move {
        let mut interval = time::interval(poll);
        // Start ticking after one full period (reduce immediate burst on startup)
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

        // last sent values (None -> force first send)
        let mut last_playing: Option<bool> = None;
        let mut last_mode: Option<String> = None;

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let body = json!({
                        "id": 1,
                        "method": "slim.request",
                        "params": [ cfg.mac, ["status","-",1] ]
                    });

                    let (playing, mode) = match client.post(&url).json(&body).send().await {
                        Ok(resp) => {
                            match resp.json::<serde_json::Value>().await {
                                Ok(v) => {
                                    let mode = v.get("result")
                                        .and_then(|r| r.get("mode"))
                                        .and_then(|m| m.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    (mode == "play", mode)
                                }
                                Err(e) => {
                                    warn!("status beacon: bad JSON: {e}");
                                    (false, String::new())
                                }
                            }
                        }
                        Err(e) => {
                            // Treat failures as "not playing" so visualizer will idle
                            warn!("status beacon: request error: {e}");
                            (false, String::new())
                        }
                    };

                    // Only send when something changed (initially last_* are None -> send)
                    if last_playing != Some(playing) || last_mode.as_deref() != Some(mode.as_str()) {
                        let _ = evt_tx.try_send(PlayingEvent {
                            playing,
                            mode: mode.clone(),
                            ts: Instant::now(),
                        });
                        last_playing = Some(playing);
                        last_mode = Some(mode);
                    }
                }

                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(StatusCmd::Shutdown) | None => {
                            debug!("status beacon: shutdown");
                            break;
                        }
                    }
                }
            }
        }
    });

    Ok((cmd_tx, evt_rx))
}
