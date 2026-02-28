//! Recording module: record and playback browser automation.

pub mod manager;
pub mod schema;

pub use manager::RecordingManager;
pub use schema::{Recording, RecordedAction, RecordedActionType, RecordingSession};
