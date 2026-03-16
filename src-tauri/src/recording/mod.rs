//! Recording module: record and playback browser automation.

pub mod manager;
pub mod playback;
pub mod schema;
pub mod session;

pub use manager::RecordingManager;
pub use playback::PlaybackResult;
pub use schema::{Recording, RecordedAction, RecordedActionType, RecordingSession, RecordingSessionInfo};
pub use session::RecordingSessionManager;
