use std::sync::atomic::{AtomicU16, Ordering};

static CDP_PORT_COUNTER: AtomicU16 = AtomicU16::new(9222);

/// Allocate the next available CDP remote-debugging port.
/// Starts at 9222 and increments; wraps around at 65500.
pub fn allocate_cdp_port() -> u16 {
    let port = CDP_PORT_COUNTER.fetch_add(1, Ordering::SeqCst);
    if port > 65500 {
        CDP_PORT_COUNTER.store(9222, Ordering::SeqCst);
        return 9222;
    }
    port
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_cdp_port_increments() {
        let p1 = allocate_cdp_port();
        let p2 = allocate_cdp_port();
        assert_eq!(p2, p1 + 1);
    }
}
