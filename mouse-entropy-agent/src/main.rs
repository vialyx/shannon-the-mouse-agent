use anyhow::Result;
use crossbeam_channel::unbounded;
use mouse_entropy_agent::{
    buffer::{MouseSample, RollingBuffer},
    capture,
    config::AppConfig,
    emitter::{current_timestamp_ms, Emitter, TelemetryEvent},
    entropy, scorer,
};
use std::time::Duration;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = AppConfig::load().unwrap_or_else(|e| {
        eprintln!(
            "Warning: could not load config.toml ({}), using defaults",
            e
        );
        AppConfig::default()
    });

    let session_id = Uuid::new_v4().to_string();
    eprintln!(
        "Mouse Entropy Agent v{} | session={} | window={}ms | bins={}",
        env!("CARGO_PKG_VERSION"),
        session_id,
        cfg.window.duration_ms,
        cfg.window.bins,
    );

    let (mouse_tx, mouse_rx) = unbounded::<MouseSample>();

    // Spawn the blocking rdev listener on a dedicated thread.
    tokio::task::spawn_blocking(move || {
        if let Err(e) = capture::start_capture(mouse_tx) {
            eprintln!("Mouse capture terminated: {}", e);
        }
    });

    let scorer_inst = scorer::Scorer {
        medium: cfg.thresholds.medium,
        high: cfg.thresholds.high,
        critical: cfg.thresholds.critical,
    };

    let mut emitter = Emitter::new(
        cfg.emit.stdout,
        cfg.emit.http_endpoint.clone(),
        cfg.emit.http_interval_ms,
        session_id.clone(),
    );

    let mut buffer = RollingBuffer::new(cfg.window.duration_ms);
    let window_duration = Duration::from_millis(cfg.window.duration_ms);
    let mut ticker = tokio::time::interval(window_duration);

    // Keep the last 10 windows for the optional Anthropic anomaly explanation.
    let mut recent_windows: std::collections::VecDeque<TelemetryEvent> =
        std::collections::VecDeque::with_capacity(11);

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            eprintln!("Received Ctrl-C – shutting down.");
        }
        _ = async {
            loop {
                ticker.tick().await;

                // Drain all pending mouse events from the capture thread.
                while let Ok(sample) = mouse_rx.try_recv() {
                    buffer.push(sample);
                }

                let samples = buffer.window_samples();
                if let Some(result) = entropy::compute_risk(
                    samples,
                    cfg.window.bins,
                    cfg.scoring.alpha,
                    cfg.scoring.beta,
                ) {
                    let risk_level = scorer_inst.classify(result.risk_score);

                    let mut event = TelemetryEvent {
                        ts: current_timestamp_ms(),
                        window_ms: cfg.window.duration_ms,
                        sample_count: result.sample_count,
                        entropy_raw: (result.entropy_raw * 1000.0).round() / 1000.0,
                        entropy_norm: (result.entropy_norm * 1000.0).round() / 1000.0,
                        velocity_mean: (result.velocity_mean * 10.0).round() / 10.0,
                        velocity_jitter: (result.velocity_jitter * 10.0).round() / 10.0,
                        risk_score: (result.risk_score * 1000.0).round() / 1000.0,
                        risk_level: risk_level.to_string(),
                        session_id: session_id.clone(),
                        anomaly_explanation: None,
                    };

                    // Optional: AI-powered anomaly explanation.
                    if result.risk_score >= cfg.thresholds.critical {
                        if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
                            let context: Vec<TelemetryEvent> =
                                recent_windows.iter().cloned().collect();
                            match mouse_entropy_agent::emitter::get_anomaly_explanation(
                                &api_key, &context,
                            )
                            .await
                            {
                                Ok(explanation) => event.anomaly_explanation = Some(explanation),
                                Err(e) => eprintln!("Anomaly explanation error: {}", e),
                            }
                        }
                    }

                    if let Err(e) = emitter.emit(&event).await {
                        eprintln!("Emit error: {}", e);
                    }

                    // Update the sliding window of recent events.
                    recent_windows.push_back(event);
                    if recent_windows.len() > 10 {
                        recent_windows.pop_front();
                    }
                }
            }
        } => {}
    }

    Ok(())
}
