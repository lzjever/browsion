//! Workflow engine: multi-step task automation.

pub mod executor;
pub mod manager;
pub mod schema;

pub use executor::WorkflowExecutor;
pub use manager::WorkflowManager;
pub use schema::*;
