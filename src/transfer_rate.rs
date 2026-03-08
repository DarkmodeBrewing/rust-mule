use std::collections::VecDeque;
use std::time::{Duration, Instant};

const WINDOW_5S: Duration = Duration::from_secs(5);
const WINDOW_30S: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TransferRateSnapshot {
    pub rate_bps_5s: u64,
    pub rate_bps_30s: u64,
}

#[derive(Debug, Clone, Default)]
pub struct RollingTransferRate {
    samples: VecDeque<ByteSample>,
}

#[derive(Debug, Clone, Copy)]
struct ByteSample {
    at: Instant,
    bytes: u64,
}

impl RollingTransferRate {
    pub fn note_bytes(&mut self, bytes: u64) {
        self.note_bytes_at(Instant::now(), bytes);
    }

    pub fn note_bytes_at(&mut self, at: Instant, bytes: u64) {
        if bytes == 0 {
            return;
        }
        self.samples.push_back(ByteSample { at, bytes });
        self.prune(at);
    }

    pub fn snapshot(&mut self) -> TransferRateSnapshot {
        self.snapshot_at(Instant::now())
    }

    pub fn snapshot_at(&mut self, now: Instant) -> TransferRateSnapshot {
        self.prune(now);
        TransferRateSnapshot {
            rate_bps_5s: rate_for_window(&self.samples, now, WINDOW_5S),
            rate_bps_30s: rate_for_window(&self.samples, now, WINDOW_30S),
        }
    }

    fn prune(&mut self, now: Instant) {
        while let Some(sample) = self.samples.front() {
            if now.duration_since(sample.at) > WINDOW_30S {
                self.samples.pop_front();
            } else {
                break;
            }
        }
    }
}

fn rate_for_window(samples: &VecDeque<ByteSample>, now: Instant, window: Duration) -> u64 {
    let bytes = samples
        .iter()
        .filter(|sample| now.duration_since(sample.at) <= window)
        .map(|sample| sample.bytes)
        .sum::<u64>();
    bytes / window.as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolling_transfer_rate_respects_5s_and_30s_windows() {
        let start = Instant::now();
        let mut rate = RollingTransferRate::default();
        rate.note_bytes_at(start, 500);
        rate.note_bytes_at(start + Duration::from_secs(4), 500);
        rate.note_bytes_at(start + Duration::from_secs(20), 3000);

        let snapshot = rate.snapshot_at(start + Duration::from_secs(20));
        assert_eq!(snapshot.rate_bps_5s, 3000 / 5);
        assert_eq!(snapshot.rate_bps_30s, 4000 / 30);
    }

    #[test]
    fn rolling_transfer_rate_prunes_samples_older_than_30s() {
        let start = Instant::now();
        let mut rate = RollingTransferRate::default();
        rate.note_bytes_at(start, 3000);
        rate.note_bytes_at(start + Duration::from_secs(31), 1500);

        let snapshot = rate.snapshot_at(start + Duration::from_secs(31));
        assert_eq!(snapshot.rate_bps_5s, 1500 / 5);
        assert_eq!(snapshot.rate_bps_30s, 1500 / 30);
    }
}
