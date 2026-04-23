use std::collections::VecDeque;

/// A single mouse-movement sample.
#[derive(Debug, Clone)]
pub struct MouseSample {
    pub x: f64,
    pub y: f64,
    /// Milliseconds since Unix epoch.
    pub timestamp_ms: u64,
}

/// Rolling circular buffer that retains only samples within the configured
/// time window.  Eviction is driven by each incoming sample's timestamp so
/// that unit/integration tests with synthetic timestamps work correctly.
pub struct RollingBuffer {
    samples: VecDeque<MouseSample>,
    window_ms: u64,
}

impl RollingBuffer {
    pub fn new(window_ms: u64) -> Self {
        Self {
            samples: VecDeque::new(),
            window_ms,
        }
    }

    /// Push a new sample and evict any samples outside the rolling window.
    pub fn push(&mut self, sample: MouseSample) {
        let latest_ts = sample.timestamp_ms;
        self.samples.push_back(sample);

        // Remove samples older than `window_ms` relative to the newest sample.
        let cutoff = latest_ts.saturating_sub(self.window_ms);
        while let Some(front) = self.samples.front() {
            if front.timestamp_ms < cutoff {
                self.samples.pop_front();
            } else {
                break;
            }
        }
    }

    /// Borrow the current window of samples.
    pub fn window_samples(&self) -> &VecDeque<MouseSample> {
        &self.samples
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_old_samples() {
        let mut buf = RollingBuffer::new(500);
        // t=0..499 → all within window when latest is 499
        for t in 0..500u64 {
            buf.push(MouseSample { x: 0.0, y: 0.0, timestamp_ms: t });
        }
        assert_eq!(buf.len(), 500);

        // Push t=1000 → cutoff=500 → t=0..499 all evicted
        buf.push(MouseSample { x: 0.0, y: 0.0, timestamp_ms: 1000 });
        // Only t=1000 remains
        assert_eq!(buf.len(), 1);
    }

    #[test]
    fn retains_samples_within_window() {
        let mut buf = RollingBuffer::new(500);
        for t in (0u64..=1000).step_by(10) {
            buf.push(MouseSample { x: 0.0, y: 0.0, timestamp_ms: t });
        }
        // Window is 500ms; latest is 1000 → cutoff 500 → keep t=500..=1000 (51 samples)
        assert_eq!(buf.len(), 51);
    }
}
