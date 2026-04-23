/// Integration tests simulate synthetic mouse events through the full
/// buffer → entropy → scorer pipeline without requiring a live display.
use mouse_entropy_agent::{
    buffer::{MouseSample, RollingBuffer},
    entropy::compute_risk,
    scorer::{RiskLevel, Scorer},
};
use std::time::{SystemTime, UNIX_EPOCH};

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn scorer() -> Scorer {
    Scorer { medium: 0.3, high: 0.6, critical: 0.8 }
}

/// Push `count` samples along a circular path into a rolling buffer.
/// Returns the buffer after all pushes.
fn circular_buffer(count: u64, radius: f64, step_ms: u64, window_ms: u64) -> RollingBuffer {
    let base = now_ms();
    let mut buf = RollingBuffer::new(window_ms);
    for i in 0..count {
        let a = 2.0 * std::f64::consts::PI * i as f64 / 100.0;
        buf.push(MouseSample {
            x: 500.0 + radius * a.cos(),
            y: 500.0 + radius * a.sin(),
            timestamp_ms: base + i * step_ms,
        });
    }
    buf
}

// ── 1 000 event smoke test ───────────────────────────────────────────────────

#[test]
fn simulate_1000_circular_events_risk_in_range() {
    let buf = circular_buffer(1000, 100.0, 5, 500);
    let samples = buf.window_samples();

    // After 1000 × 5 ms = 5 s of data the 500 ms window holds ~100 samples.
    assert!(samples.len() >= 2, "window must contain at least 2 samples");

    let result = compute_risk(samples, 16, 0.6, 0.4).expect("compute_risk returned None");

    assert!(
        result.risk_score >= 0.0 && result.risk_score <= 1.0,
        "risk_score out of [0,1]: {}",
        result.risk_score
    );
    assert!(
        result.entropy_norm > 0.5,
        "circular motion entropy should be > 0.5, got {}",
        result.entropy_norm
    );

    // Circular motion hits at least MEDIUM risk.
    let level = scorer().classify(result.risk_score);
    assert!(
        matches!(level, RiskLevel::Medium | RiskLevel::High | RiskLevel::Critical),
        "expected MEDIUM or above, got {:?}",
        level
    );
}

// ── straight-line (robotic) movement ────────────────────────────────────────

#[test]
fn simulate_1000_straight_line_events_low_risk() {
    let base = now_ms();
    let mut buf = RollingBuffer::new(500);
    for i in 0u64..1000 {
        buf.push(MouseSample {
            x: i as f64 * 1.0,
            y: 100.0,
            timestamp_ms: base + i * 5,
        });
    }
    let samples = buf.window_samples();
    assert!(samples.len() >= 2);

    let result = compute_risk(samples, 16, 0.6, 0.4).expect("compute_risk returned None");

    assert!(result.entropy_norm < 0.3, "straight line entropy should be low");
    assert_eq!(
        scorer().classify(result.risk_score),
        RiskLevel::Low,
        "straight line should score LOW"
    );
}

// ── randomised movement (high entropy) ──────────────────────────────────────

#[test]
fn simulate_1000_random_direction_events_elevated_entropy() {
    let base = now_ms();
    let mut buf = RollingBuffer::new(500);
    // Deterministic "random-looking" pattern: 16 evenly-spaced directions,
    // repeated 62 times + some leftover (≈ 1000 samples over 5 s).
    let dirs = 16usize;
    for i in 0u64..1000 {
        let idx = i as usize % dirs;
        let a = 2.0 * std::f64::consts::PI * idx as f64 / dirs as f64;
        buf.push(MouseSample {
            x: 300.0 + 10.0 * a.cos() * (i as f64 * 0.01 + 1.0),
            y: 300.0 + 10.0 * a.sin() * (i as f64 * 0.01 + 1.0),
            timestamp_ms: base + i * 5,
        });
    }
    let samples = buf.window_samples();
    assert!(samples.len() >= 2);

    let result = compute_risk(samples, 16, 0.6, 0.4).expect("compute_risk returned None");

    // 16-direction distribution should produce high entropy.
    assert!(
        result.entropy_norm > 0.6,
        "multi-direction pattern entropy should be > 0.6, got {}",
        result.entropy_norm
    );
}

// ── risk score is bounded for extreme inputs ─────────────────────────────────

#[test]
fn risk_score_bounded_for_extreme_velocity() {
    let base = now_ms();
    let mut buf = RollingBuffer::new(500);
    // Huge jumps every millisecond → enormous velocity jitter
    for i in 0u64..200 {
        buf.push(MouseSample {
            x: if i % 2 == 0 { 0.0 } else { 10_000.0 },
            y: if i % 2 == 0 { 0.0 } else { 10_000.0 },
            timestamp_ms: base + i,
        });
    }
    let samples = buf.window_samples();
    let result = compute_risk(samples, 16, 0.6, 0.4).expect("compute_risk returned None");
    assert!(result.risk_score <= 1.0, "risk_score must never exceed 1.0");
    assert!(result.risk_score >= 0.0);
}

// ── scorer threshold boundaries ──────────────────────────────────────────────

#[test]
fn scorer_boundary_values() {
    let s = scorer();
    assert_eq!(s.classify(0.0), RiskLevel::Low);
    assert_eq!(s.classify(0.3), RiskLevel::Medium);
    assert_eq!(s.classify(0.6), RiskLevel::High);
    assert_eq!(s.classify(0.8), RiskLevel::Critical);
    assert_eq!(s.classify(1.0), RiskLevel::Critical);
}
