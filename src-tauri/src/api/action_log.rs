//! In-memory action log: records every HTTP API call with timing and outcome.
//! Entries are also appended to daily JSONL files under ~/.browsion/logs/.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;

const MAX_ENTRIES: usize = 2000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionEntry {
    pub id: String,         // uuid v4
    pub ts: u64,            // Unix ms
    pub profile_id: String, // empty string when not profile-scoped
    pub tool: String,       // e.g. "navigate", "screenshot"
    pub duration_ms: u64,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct ActionLog {
    buffer: Arc<Mutex<VecDeque<ActionEntry>>>,
}

impl ActionLog {
    pub fn new() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_ENTRIES))),
        }
    }

    pub fn push(&self, entry: ActionEntry) {
        let mut buf = self.buffer.lock();
        if buf.len() >= MAX_ENTRIES {
            buf.pop_front();
        }
        buf.push_back(entry);
    }

    /// Return entries filtered by optional profile_id, newest-first, up to `limit`.
    pub fn get_filtered(&self, profile_id: Option<&str>, limit: usize) -> Vec<ActionEntry> {
        let buf = self.buffer.lock();
        buf.iter()
            .rev()
            .filter(|e| {
                profile_id
                    .map(|id| e.profile_id == id)
                    .unwrap_or(true)
            })
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn clear(&self, profile_id: Option<&str>) {
        let mut buf = self.buffer.lock();
        match profile_id {
            Some(id) => buf.retain(|e| e.profile_id != id),
            None => buf.clear(),
        }
    }
}

impl Default for ActionLog {
    fn default() -> Self {
        Self::new()
    }
}

/// Append one entry to the daily log file asynchronously (fire-and-forget).
pub async fn append_to_file(entry: &ActionEntry) {
    let ts = entry.ts;
    // ts is Unix ms → convert to naive date
    let secs = ts / 1000;
    let date = {
        let secs_i64 = secs as i64;
        let days = secs_i64 / 86400;
        // days since unix epoch 1970-01-01
        let (y, m, d) = days_to_ymd(days);
        format!("{:04}-{:02}-{:02}", y, m, d)
    };

    let log_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".browsion")
        .join("logs");

    let line = match serde_json::to_string(entry) {
        Ok(s) => s + "\n",
        Err(_) => return,
    };

    tokio::spawn(async move {
        if let Err(e) = tokio::fs::create_dir_all(&log_dir).await {
            tracing::warn!("Failed to create log dir: {}", e);
            return;
        }
        let path = log_dir.join(format!("{}.jsonl", date));
        use tokio::io::AsyncWriteExt;
        match tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
        {
            Ok(mut f) => {
                let _ = f.write_all(line.as_bytes()).await;
            }
            Err(e) => tracing::warn!("Failed to write action log: {}", e),
        }
    });
}

/// Convert days-since-unix-epoch to (year, month, day). Gregorian calendar.
fn days_to_ymd(days: i64) -> (i64, i64, i64) {
    // Algorithm: civil from days (Howard Hinnant)
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
