//! CDP (Chrome DevTools Protocol) browser control and session management.

pub mod cdp;
pub mod session;
pub mod types;

pub use cdp::CDPClient;
pub use session::SessionManager;
pub use types::*;
