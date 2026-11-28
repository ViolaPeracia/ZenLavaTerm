//! Input processing and domain action translation.

pub mod coords;
pub mod keyboard;
pub mod mouse;

pub use coords::terminal_to_sim_coords;
pub use keyboard::{map_key_event, map_key_event_with_ripple, ripple_origin_for_char, Action};
pub use mouse::MouseTracker;
