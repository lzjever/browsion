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

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to build a minimal ActionEntry for testing.
    fn make_entry(id: &str, profile_id: &str, tool: &str, ts: u64) -> ActionEntry {
        ActionEntry {
            id: id.to_string(),
            ts,
            profile_id: profile_id.to_string(),
            tool: tool.to_string(),
            duration_ms: 10,
            success: true,
            error: None,
        }
    }

    // -----------------------------------------------------------------------
    // ActionLog::push + get_filtered — basic ordering
    // -----------------------------------------------------------------------

    #[test]
    fn test_push_and_get_filtered_order() {
        let log = ActionLog::new();
        log.push(make_entry("1", "p1", "navigate", 1000));
        log.push(make_entry("2", "p1", "click", 2000));
        log.push(make_entry("3", "p1", "screenshot", 3000));

        let entries = log.get_filtered(None, 100);
        assert_eq!(entries.len(), 3, "should have 3 entries");
        // Newest-first: ts 3000, 2000, 1000
        assert_eq!(entries[0].ts, 3000);
        assert_eq!(entries[1].ts, 2000);
        assert_eq!(entries[2].ts, 1000);
    }

    // -----------------------------------------------------------------------
    // get_filtered with profile_id filter
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_filtered_by_profile() {
        let log = ActionLog::new();
        log.push(make_entry("1", "p1", "navigate", 1000));
        log.push(make_entry("2", "p2", "click", 2000));
        log.push(make_entry("3", "p1", "screenshot", 3000));
        log.push(make_entry("4", "p2", "hover", 4000));

        let p1_entries = log.get_filtered(Some("p1"), 100);
        assert_eq!(p1_entries.len(), 2);
        assert!(p1_entries.iter().all(|e| e.profile_id == "p1"));
        // Newest-first among p1 entries
        assert_eq!(p1_entries[0].ts, 3000);
        assert_eq!(p1_entries[1].ts, 1000);

        let p2_entries = log.get_filtered(Some("p2"), 100);
        assert_eq!(p2_entries.len(), 2);
        assert!(p2_entries.iter().all(|e| e.profile_id == "p2"));
    }

    // -----------------------------------------------------------------------
    // get_filtered with limit
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_filtered_with_limit() {
        let log = ActionLog::new();
        for i in 0..10u64 {
            log.push(make_entry(&i.to_string(), "p1", "click", i * 100));
        }

        let entries = log.get_filtered(None, 3);
        assert_eq!(entries.len(), 3, "limit should be respected");
        // Newest-first: ts 900, 800, 700
        assert_eq!(entries[0].ts, 900);
        assert_eq!(entries[1].ts, 800);
        assert_eq!(entries[2].ts, 700);
    }

    // -----------------------------------------------------------------------
    // clear(None) — clears all entries
    // -----------------------------------------------------------------------

    #[test]
    fn test_clear_all() {
        let log = ActionLog::new();
        log.push(make_entry("1", "p1", "navigate", 1000));
        log.push(make_entry("2", "p2", "click", 2000));

        log.clear(None);
        let entries = log.get_filtered(None, 100);
        assert!(entries.is_empty(), "all entries should be cleared");
    }

    // -----------------------------------------------------------------------
    // clear(Some("p1")) — clears only p1, p2 remains
    // -----------------------------------------------------------------------

    #[test]
    fn test_clear_by_profile() {
        let log = ActionLog::new();
        log.push(make_entry("1", "p1", "navigate", 1000));
        log.push(make_entry("2", "p2", "click", 2000));
        log.push(make_entry("3", "p1", "screenshot", 3000));

        log.clear(Some("p1"));

        let remaining = log.get_filtered(None, 100);
        assert_eq!(remaining.len(), 1, "only p2 entry should remain");
        assert_eq!(remaining[0].profile_id, "p2");

        let p1_entries = log.get_filtered(Some("p1"), 100);
        assert!(p1_entries.is_empty(), "p1 entries should all be gone");
    }

    // -----------------------------------------------------------------------
    // Capacity limit: push MAX_ENTRIES+10, verify len=MAX_ENTRIES and newest retained
    // -----------------------------------------------------------------------

    #[test]
    fn test_capacity_limit() {
        let log = ActionLog::new();
        let total = MAX_ENTRIES + 10;
        for i in 0..total as u64 {
            log.push(make_entry(&i.to_string(), "p1", "click", i));
        }

        let entries = log.get_filtered(None, MAX_ENTRIES + 100);
        assert_eq!(entries.len(), MAX_ENTRIES, "buffer should be capped at MAX_ENTRIES");

        // Newest entries are retained: highest ts values (total-1 down to 10)
        let newest_ts = entries[0].ts;
        let oldest_ts = entries[entries.len() - 1].ts;
        assert_eq!(newest_ts, (total as u64) - 1, "newest entry should have highest ts");
        assert_eq!(oldest_ts, 10, "oldest retained should be entry #10 (first 10 evicted)");
    }

    // -----------------------------------------------------------------------
    // days_to_ymd tests — values verified by hand computation
    // -----------------------------------------------------------------------

    // day 0 = Unix epoch = 1970-01-01
    #[test]
    fn test_days_to_ymd_epoch() {
        assert_eq!(days_to_ymd(0), (1970, 1, 1));
    }

    // day 365 = 1971-01-01 (1970 is not a leap year, so exactly 365 days)
    #[test]
    fn test_days_to_ymd_one_year() {
        assert_eq!(days_to_ymd(365), (1971, 1, 1));
    }

    // day 11016 = 2000-02-29 (verified: 30*365+7=10957 to 2000-01-01, +31=10988 to Feb 1, +28=11016 to Feb 29)
    #[test]
    fn test_days_to_ymd_leap_day_2000() {
        assert_eq!(days_to_ymd(11016), (2000, 2, 29));
    }

    // day 20513 = 2026-03-01 (verified: 10957+9497=20454 to 2026-01-01, +59 days (Jan31+Feb28)=20513)
    #[test]
    fn test_days_to_ymd_2026_03_01() {
        assert_eq!(days_to_ymd(20513), (2026, 3, 1));
    }
}
