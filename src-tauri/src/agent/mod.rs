//! AI Agent module for browser automation
//!
//! This module provides an AI-powered browser automation feature that allows
//! users to describe tasks in natural language and have an AI agent execute them.
//!
//! # Architecture
//!
//! - `cdp`: Chrome DevTools Protocol client for browser control
//! - `llm`: LLM/VLM API clients for decision making
//! - `action`: Action execution (click, type, navigate, etc.)
//! - `engine`: Main agent loop orchestration
//! - `types`: Type definitions for the agent system

pub mod action;
pub mod cdp;
pub mod engine;
pub mod llm;
pub mod types;

pub use action::{execute_action, ActionResult};
pub use cdp::CDPClient;
pub use engine::AgentEngine;
pub use types::*;
