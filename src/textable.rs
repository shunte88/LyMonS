/*
 *  textable.rs
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
// textable.rs
#[allow(dead_code)]
use embedded_graphics::{
    mono_font::{
        ascii::{
            FONT_5X8},
        MonoFont,
    },
    prelude::*,
};

use std::sync::Arc;
use tokio::sync::Mutex as TokMutex; // Aliased tokio::sync::Mutex
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use log::{info, debug}; // Add logging

const SCROLL_LEFT: i8 = -1;
const SCROLL_RIGHT: i8 = 1;
pub const GAP_BETWEEN_LOOP_TEXT_FIXED: i32 = 12; // Fixed gap for continuous loop

/// Enum for scroll modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollMode {
    Static,
    ScrollLeft,
    ScrollCylon,
}

/// Internal shared state for the `TextScroller` task.
pub struct State { // Made public so OledDisplay can read it
    pub text: String,
    pub scroll_mode: ScrollMode,
    pub font: MonoFont<'static>, // Stored here to be accessible to the task
    pub text_width: u32, // NEW: Text width, updated by OledDisplay
    pub stop_flag: bool,
    pub current_offset_float: f32, // Use f32 for smoother scrolling calculation
    pub last_drawn_x_rounded: i32, // Store the last rounded X position that was drawn
    pub direction: i8, // -1 for left, 1 for right (for cylon)
    pub paused: bool, // For pause states in scrolling
    pub has_paused: bool, // For pause states in scrolling
}

pub fn transform_scroll_mode(scroll_mode: &str) -> ScrollMode {
    match scroll_mode {
        "cylon" =>  ScrollMode::ScrollCylon,
        "loop" => ScrollMode::ScrollLeft,
        "loopleft" => ScrollMode::ScrollLeft,
        _ => ScrollMode::Static,
    }
}

impl Default for State {
    fn default() -> Self {
        State {
            text: String::new(),
            scroll_mode: ScrollMode::Static,
            font: FONT_5X8, // Default font
            text_width: 0,
            stop_flag: false,
            current_offset_float: 0.0,
            last_drawn_x_rounded: i32::MIN, // Initialize to a value that will force a redraw on first tick
            direction: SCROLL_LEFT,
            paused: false,
            has_paused: false,
        }
    }
}

/// A struct that manages a single line of text scrolling *state*.
/// It does not interact with the display hardware directly.
pub struct TextScroller {
    pub name: String,
    pub top_left: Point, // Public so OledDisplay can get position
    pub width: u32, // Display region width
    pub state: Arc<TokMutex<State>>, // Shared state for the scrolling task
    task_handle: Option<JoinHandle<()>>, // Handle to the spawned async task
}

impl TextScroller {
    /// Creates a new `TextScroller`. It does NOT start the scrolling task immediately.
    /// The task must be explicitly started by calling `start()`.

    pub fn new(
        name: String,
        top_left: Point,
        width: u32,
        initial_text: String,
        initial_font: MonoFont<'static>,
        initial_scroll_mode: ScrollMode,
    ) -> Self {
        let state = Arc::new(TokMutex::new(State {
            text: initial_text,
            scroll_mode: initial_scroll_mode,
            font: initial_font,
            text_width: 0, // Will be set by OledDisplay
            stop_flag: true, // Initially stopped
            current_offset_float: 0.0,
            last_drawn_x_rounded: i32::MIN,
            direction: SCROLL_LEFT,
            paused: false,
            has_paused: false,
        }));

        Self {
            name, // for debug purposes
            top_left,
            width,
            state,
            task_handle: None,
        }
    }

    /// Spawns the async scrolling task if it's not already running.
    fn spawn_task(&mut self) {
        if self.task_handle.is_some() {
            info!("{} task is already running. Not spawning new.", self.name);
            return;
        }
        let name = self.name.clone();
        let display_width = self.width;
        let state = self.state.clone();
        
        const SCROLL_AMOUNT_PER_TICK: f32 = 0.5; // Fixed scroll amount per tick
        const PAUSE_DURATION_MILLIS: u64 = 1500;  // Fixed pause duration
        const NORMAL_DURATION_MILLIS: u64 = 30;  // Fixed pause duration

        let handle = tokio::spawn(async move {
            loop {
                let mut s = state.lock().await;
                if s.stop_flag {
                    info!("{} task exiting.", name);
                    // Do NOT set task_handle = None here, as `self.task_handle` is owned by `TextScroller`
                    // and this is the spawned task. `TextScroller::stop()` handles clearing it.
                    drop(s); // Release lock before break
                    break;
                }

                let text_width = s.text_width; // Get text width from state
                let mode = s.scroll_mode;
                let max_offset_right = display_width as f32 - text_width as f32;

                // If somehow in Static mode or text fits and task is running, stop it
                if mode == ScrollMode::Static || text_width <= display_width {
                    if !s.stop_flag { // Only log and set stop_flag if not already stopped
                        debug!("{} text fits or mode is static.", name);
                        s.stop_flag = true;
                        // Reset offset to centered for static mode or 0 for fitting scroll mode
                        s.current_offset_float = ((display_width - text_width) / 2) as f32;
                        s.last_drawn_x_rounded = i32::MIN; // Force redraw to static position
                    }
                    drop(s); // Release lock before break
                    break; // Exit the loop and terminate the task
                }

                match mode {
                    ScrollMode::Static => {
                        // we should never get here
                        drop(s);
                        break; // Exit the loop and terminate the task
                    }
                    ScrollMode::ScrollLeft => {
                        s.current_offset_float -= SCROLL_AMOUNT_PER_TICK;
                        if s.current_offset_float <= -(text_width as f32 + GAP_BETWEEN_LOOP_TEXT_FIXED as f32) {
                            s.current_offset_float = 0.0; // emulkate wrap around
                        }
                    }
                    ScrollMode::ScrollCylon => {
                        
                        s.current_offset_float += s.direction as f32 * SCROLL_AMOUNT_PER_TICK;

                        if s.direction == SCROLL_LEFT {
                            if s.current_offset_float == 0.0 {
                                if !s.has_paused {
                                    s.paused = true;
                                }
                            }
                            // if right edge fully visible
                            if s.current_offset_float <= max_offset_right {
                                s.direction = SCROLL_RIGHT;
                            } // Reverse direction to right

                        } else if s.direction == SCROLL_RIGHT {
                            if s.current_offset_float == 0.0 && s.has_paused {
                                s.direction = SCROLL_LEFT
                            }
                        } // Reverse direction to left
                    }
                        
                }

                // Update the last_drawn_x_rounded in state so OledDisplay knows to redraw.
                let new_rounded_x = (s.current_offset_float).round() as i32;
                if new_rounded_x != s.last_drawn_x_rounded {
                    s.last_drawn_x_rounded = new_rounded_x;
                }
                
                let pausing = s.paused;
                drop(s); // Release lock before awaiting sleep
                let mut sleep_millis = NORMAL_DURATION_MILLIS;
                if pausing {
                    sleep_millis = PAUSE_DURATION_MILLIS;
                }

                sleep(Duration::from_millis(sleep_millis)).await;

                if pausing {
                    let mut s = state.lock().await;
                    s.paused = false;
                    s.has_paused = true;
                    //s.current_offset_float = -SCROLL_AMOUNT_PER_TICK;
                    drop(s); // Release lock before awaiting sleep
                }
            }
        });

        self.task_handle = Some(handle);

    }

    /// Updates the text, scroll mode, and text width in the scroller's state.
    /// Does NOT start or stop the internal task. OledDisplay handles that.
    pub async fn update_content(&mut self, new_text: String, new_mode: ScrollMode, new_text_width: u32) {
        let mut s = self.state.lock().await;
        if s.text == new_text && s.scroll_mode == new_mode && s.text_width == new_text_width {
            return;
        }
        let display_width = self.width;
        debug!("{} updating state. {:?} width {}", self.name, new_mode, new_text_width);

        s.text = new_text;
        s.scroll_mode = new_mode;
        s.text_width = new_text_width; // Update text width
        if new_text_width <= display_width {
            s.current_offset_float = ((display_width - new_text_width) / 2) as f32;
            s.scroll_mode = ScrollMode::Static;
        } else {
            if new_mode == ScrollMode::ScrollCylon || new_mode == ScrollMode::ScrollLeft {
                s.current_offset_float = display_width as f32;
                s.direction = SCROLL_LEFT;
            }
        }
        s.last_drawn_x_rounded = i32::MIN; // Force redraw by OledDisplay
        s.paused = false;
        s.has_paused = false;

    }

    /// Starts the scrolling animation task.
    pub async fn start(&mut self) {
        let mut s = self.state.lock().await;
        if s.stop_flag {
            debug!("{}. task init...", self.name);
            s.stop_flag = false;
            s.last_drawn_x_rounded = i32::MIN; // Force initial redraw
            drop(s); // Release lock before spawning
            self.spawn_task();
        } else {
            debug!("{}. is running or init...", self.name);
        }
    }

    /// Stops the scrolling animation gracefully.
    pub async fn stop(&mut self) {
        let this_ = self.name.clone();
        let mut s = self.state.lock().await;
        if !s.stop_flag {
            debug!("{} stopping...", this_);
            s.stop_flag = true; // Signal the task to stop itself gracefully
            // Reset offset and last_drawn to ensure clean redraw or static positioning
            s.current_offset_float = 0.0;
            s.last_drawn_x_rounded = i32::MIN; // Force redraw on next OledDisplay frame
        }
        drop(s); // Release lock before aborting task
        if let Some(handle) = self.task_handle.take() {
            handle.abort(); // Abort the old task (non-blocking)
            debug!("{} task aborted.", self.name);
        }
    }

}

// Implement Drop to ensure the task is stopped when the TextScroller instance is dropped
impl Drop for TextScroller {
    fn drop(&mut self) {
        // Since `stop` is async, we can't call it directly in `Drop`.
        // The `main.rs` or `OledDisplay` should explicitly stop scrollers on shutdown.
        // For scenarios where `drop` is called implicitly (e.g., Vec clean-up),
        // the abort() in `stop()` will be called if `task_handle` exists.
        // The main loop should manage explicit shutdown via `stop()`.
        if let Some(handle) = self.task_handle.take() {
            handle.abort(); // Ensure task is aborted on drop if not already stopped
            debug!("TextScroller dropped, ensuring task is aborted.");
        }
    }
}
