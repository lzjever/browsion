//! Recording module: record and playback browser automation.

pub mod manager;
pub mod schema;
pub mod session;

pub use manager::RecordingManager;
pub use schema::{Recording, RecordedAction, RecordedActionType, RecordingSession, RecordingSessionInfo};
pub use session::RecordingSessionManager;
