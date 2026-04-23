use crate::buffer::MouseSample;
use std::collections::VecDeque;

/// Output of the per-window entropy computation.
#[derive(Debug, Clone)]
pub struct EntropyResult {
    pub entropy_raw: f64,
    pub entropy_norm: f64,
    pub velocity_mean: f64,
    pub velocity_jitter: f64,
    pub risk_score: f64,
    pub sample_count: usize,
}

/// Compute Shannon entropy, velocity statistics, and a combined risk score
/// from a rolling window of mouse samples.
///
/// Returns `None` when the window contains fewer than 2 samples (no vectors
/// to analyse).
pub fn compute_risk(
    samples: &VecDeque<MouseSample>,
    bins: usize,
    alpha: f64,
    beta: f64,
) -> Option<EntropyResult> {
    if samples.len() < 2 {
        return None;
    }

    let count = samples.len();
    let mut velocities: Vec<f64> = Vec::with_capacity(count - 1);
    let mut angles: Vec<f64> = Vec::with_capacity(count - 1);

    let mut prev = samples.front()?;
    for curr in samples.iter().skip(1) {
        let dx = curr.x - prev.x;
        let dy = curr.y - prev.y;
        // Clamp dt to at least 1 ms so we never divide by zero.
        let dt = (curr.timestamp_ms as f64 - prev.timestamp_ms as f64).max(1.0);

        let dist = (dx * dx + dy * dy).sqrt();
        velocities.push(dist / dt);

        // atan2 returns [-π, π]; map to [0°, 360°).
        let mut angle_deg = dy.atan2(dx).to_degrees();
        if angle_deg < 0.0 {
            angle_deg += 360.0;
        }
        angles.push(angle_deg);

        prev = curr;
    }

    // Quantise direction angles into `bins` equal-width buckets.
    let mut bin_counts = vec![0usize; bins];
    for &angle in &angles {
        let bin = ((angle / 360.0) * bins as f64).floor() as usize;
        bin_counts[bin.min(bins - 1)] += 1;
    }

    // Shannon entropy: H = -Σ p(b) · log₂(p(b))
    let total = angles.len() as f64;
    let entropy_raw: f64 = bin_counts
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / total;
            -p * p.log2()
        })
        .sum();

    let max_entropy = (bins as f64).log2();
    let entropy_norm = if max_entropy > 0.0 {
        (entropy_raw / max_entropy).min(1.0)
    } else {
        0.0
    };

    // Velocity statistics.
    let velocity_mean = velocities.iter().sum::<f64>() / velocities.len() as f64;
    let velocity_jitter = std_dev(&velocities);

    // Normalise jitter against a practical ceiling (200 px/ms ≈ extremely fast).
    const MAX_JITTER: f64 = 200.0;
    let norm_jitter = (velocity_jitter / MAX_JITTER).min(1.0);

    let risk_score = (alpha * entropy_norm + beta * norm_jitter).min(1.0);

    Some(EntropyResult {
        entropy_raw,
        entropy_norm,
        velocity_mean,
        velocity_jitter,
        risk_score,
        sample_count: count,
    })
}

/// Population standard deviation.
fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    variance.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::MouseSample;

    fn sample(x: f64, y: f64, t: u64) -> MouseSample {
        MouseSample {
            x,
            y,
            timestamp_ms: t,
        }
    }

    fn deque(v: Vec<MouseSample>) -> VecDeque<MouseSample> {
        v.into_iter().collect()
    }

    // ── boundary conditions ──────────────────────────────────────────────────

    #[test]
    fn returns_none_for_single_sample() {
        let q = deque(vec![sample(0.0, 0.0, 0)]);
        assert!(compute_risk(&q, 16, 0.6, 0.4).is_none());
    }

    #[test]
    fn returns_none_for_empty() {
        let q: VecDeque<MouseSample> = VecDeque::new();
        assert!(compute_risk(&q, 16, 0.6, 0.4).is_none());
    }

    // ── low-entropy patterns ─────────────────────────────────────────────────

    #[test]
    fn straight_line_right_has_low_entropy() {
        // All deltas in the same direction → one occupied bin → H ≈ 0
        let q: VecDeque<_> = (0..100)
            .map(|i| sample(i as f64 * 10.0, 0.0, i as u64 * 10))
            .collect();
        let r = compute_risk(&q, 16, 0.6, 0.4).unwrap();
        assert!(
            r.entropy_norm < 0.15,
            "Expected near-zero entropy for a straight line, got {:.4}",
            r.entropy_norm
        );
        assert!(
            r.risk_score < 0.5,
            "Straight line should be low risk, got {:.4}",
            r.risk_score
        );
    }

    #[test]
    fn stationary_mouse_has_zero_velocity_and_low_entropy() {
        // All samples at the same point → Δx=Δy=0 → v=0, angle=0
        let q: VecDeque<_> = (0..30)
            .map(|i| sample(200.0, 200.0, i as u64 * 10))
            .collect();
        let r = compute_risk(&q, 16, 0.6, 0.4).unwrap();
        assert!(r.velocity_mean < 1e-6, "Stationary: velocity should be ~0");
        assert!(r.velocity_jitter < 1e-6, "Stationary: jitter should be ~0");
        assert!(r.entropy_norm < 0.15, "Stationary: entropy should be ~0");
    }

    // ── high-entropy patterns ────────────────────────────────────────────────

    #[test]
    fn uniform_circular_motion_has_high_entropy() {
        let bins = 16usize;
        let steps = bins * 10; // 10 samples per directional bin → uniform distribution
        let q: VecDeque<_> = (0..=steps)
            .map(|i| {
                let a = 2.0 * std::f64::consts::PI * i as f64 / steps as f64;
                sample(
                    500.0 + 100.0 * a.cos(),
                    500.0 + 100.0 * a.sin(),
                    i as u64 * 10,
                )
            })
            .collect();
        let r = compute_risk(&q, bins, 0.6, 0.4).unwrap();
        assert!(
            r.entropy_norm > 0.70,
            "Circular motion should have high entropy, got {:.4}",
            r.entropy_norm
        );
    }

    // ── invariants ───────────────────────────────────────────────────────────

    #[test]
    fn risk_score_is_always_in_unit_interval() {
        let q: VecDeque<_> = (0..50)
            .map(|i| {
                let a = i as f64 * 0.5;
                sample(100.0 * a.cos(), 100.0 * a.sin(), i as u64 * 5)
            })
            .collect();
        let r = compute_risk(&q, 16, 0.6, 0.4).unwrap();
        assert!(r.risk_score >= 0.0 && r.risk_score <= 1.0);
        assert!(r.entropy_norm >= 0.0 && r.entropy_norm <= 1.0);
    }

    #[test]
    fn velocity_computed_correctly_for_known_motion() {
        // 10 px right every 10 ms → v = 10/10 = 1.0 px/ms for every segment
        let q: VecDeque<_> = (0..10)
            .map(|i| sample(i as f64 * 10.0, 0.0, i as u64 * 10))
            .collect();
        let r = compute_risk(&q, 16, 0.6, 0.4).unwrap();
        assert!(
            (r.velocity_mean - 1.0).abs() < 0.01,
            "Expected velocity_mean ≈ 1.0, got {:.4}",
            r.velocity_mean
        );
        assert!(
            r.velocity_jitter < 0.01,
            "Constant-speed straight line should have near-zero jitter"
        );
    }

    #[test]
    fn two_samples_minimum_works() {
        let q = deque(vec![sample(0.0, 0.0, 0), sample(10.0, 0.0, 10)]);
        let r = compute_risk(&q, 16, 0.6, 0.4);
        assert!(r.is_some());
    }
}
