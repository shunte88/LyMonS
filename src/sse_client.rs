/*
 *  sse_client.rs
 *
 *  LyMonS - worth the squeeze
 *	(c) 2020-26 Stuart Hunter
 *
 *	Minimal SSE (Server-Sent Events) HTTP client using reqwest chunked
 *	streaming.  No external futures/stream adapters required.
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

use log::{error, info};
use reqwest::Client;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;

/// A single SSE data event.
#[derive(Debug, Clone)]
pub struct SseEvent {
    pub data: String,
}

/// Connect to an SSE endpoint, parse the event stream, and forward `data:`
/// payloads to `tx`.  Reconnects automatically with a 3-second delay on any
/// connection failure or stream termination.  Returns when `tx` is closed.
pub async fn run_sse_stream(url: String, tx: mpsc::Sender<SseEvent>) {
    let client = match Client::builder()
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            error!("sse_client: failed to build HTTP client: {e}");
            return;
        }
    };

    loop {
        info!("sse_client: connecting to {url}");

        let response = client
            .get(&url)
            .header("Accept", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "Keep-Alive")
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                info!("sse_client: connected ({})", resp.status());
                if let Err(reason) = stream_events(resp, &tx).await {
                    if reason == "closed" {
                        return; // channel dropped — stop
                    }
                    info!("sse_client: stream ended ({reason}), reconnecting…");
                }
            }
            Ok(resp) => {
                error!("sse_client: HTTP {} from {url}", resp.status());
            }
            Err(e) => {
                error!("sse_client: connection error: {e}");
            }
        }

        sleep(Duration::from_secs(3)).await;
    }
}

/// Read chunks from `response`, parse SSE blocks, and forward `data:` lines
/// to `tx`.  Returns `Ok(())` when the stream ends cleanly, or `Err(reason)`.
async fn stream_events(
    mut response: reqwest::Response,
    tx: &mpsc::Sender<SseEvent>,
) -> Result<(), &'static str> {
    let mut buffer = String::new();

    loop {
        match response.chunk().await {
            Ok(Some(bytes)) => {
                buffer.push_str(&String::from_utf8_lossy(&bytes));

                // Process all complete SSE event blocks (each terminated by \n\n).
                while let Some(pos) = buffer.find("\n\n") {
                    let block = buffer[..pos].to_string();
                    buffer = buffer[pos + 2..].to_string();

                    if let Some(data) = extract_data_line(&block) {
                        if tx.send(SseEvent { data }).await.is_err() {
                            return Err("closed");
                        }
                    }
                }
            }
            Ok(None) => return Ok(()),  // server closed connection gracefully
            Err(e) => {
                error!("sse_client: chunk error: {e}");
                return Err("chunk error");
            }
        }
    }
}

/// Extract the content of the first `data:` field in an SSE block.
fn extract_data_line(block: &str) -> Option<String> {
    for line in block.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            return Some(data.to_string());
        }
        if let Some(data) = line.strip_prefix("data:") {
            return Some(data.to_string());
        }
    }
    None
}
