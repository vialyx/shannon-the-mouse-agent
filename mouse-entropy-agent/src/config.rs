use serde::Deserialize;
use std::path::Path;

/// Top-level application configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub window: WindowConfig,
    #[serde(default)]
    pub scoring: ScoringConfig,
    #[serde(default)]
    pub emit: EmitConfig,
    #[serde(default)]
    pub thresholds: ThresholdConfig,
}

/// Rolling-window parameters.
#[derive(Debug, Clone, Deserialize)]
pub struct WindowConfig {
    #[serde(default = "default_duration_ms")]
    pub duration_ms: u64,
    #[serde(default = "default_bins")]
    pub bins: usize,
}

fn default_duration_ms() -> u64 {
    500
}
fn default_bins() -> usize {
    16
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            duration_ms: default_duration_ms(),
            bins: default_bins(),
        }
    }
}

/// Risk-score weighting parameters.
#[derive(Debug, Clone, Deserialize)]
pub struct ScoringConfig {
    #[serde(default = "default_alpha")]
    pub alpha: f64,
    #[serde(default = "default_beta")]
    pub beta: f64,
}

fn default_alpha() -> f64 {
    0.6
}
fn default_beta() -> f64 {
    0.4
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            alpha: default_alpha(),
            beta: default_beta(),
        }
    }
}

/// Telemetry emission settings.
#[derive(Debug, Clone, Deserialize)]
pub struct EmitConfig {
    #[serde(default = "default_true")]
    pub stdout: bool,
    #[serde(default)]
    pub http_endpoint: String,
    #[serde(default = "default_http_interval_ms")]
    pub http_interval_ms: u64,
}

fn default_true() -> bool {
    true
}
fn default_http_interval_ms() -> u64 {
    1000
}

impl Default for EmitConfig {
    fn default() -> Self {
        Self {
            stdout: true,
            http_endpoint: String::new(),
            http_interval_ms: default_http_interval_ms(),
        }
    }
}

/// Risk-level threshold boundaries.
#[derive(Debug, Clone, Deserialize)]
pub struct ThresholdConfig {
    #[serde(default = "default_medium")]
    pub medium: f64,
    #[serde(default = "default_high")]
    pub high: f64,
    #[serde(default = "default_critical")]
    pub critical: f64,
}

fn default_medium() -> f64 {
    0.3
}
fn default_high() -> f64 {
    0.6
}
fn default_critical() -> f64 {
    0.8
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            medium: default_medium(),
            high: default_high(),
            critical: default_critical(),
        }
    }
}

impl AppConfig {
    /// Load configuration from `config.toml` (if present) and environment
    /// variables with the `MOUSE_AGENT__` prefix.
    pub fn load() -> anyhow::Result<Self> {
        let mut builder = config::Config::builder();

        if Path::new("config.toml").exists() {
            builder = builder
                .add_source(config::File::new("config", config::FileFormat::Toml).required(false));
        }

        builder =
            builder.add_source(config::Environment::with_prefix("MOUSE_AGENT").separator("__"));

        let cfg = builder.build()?;
        Ok(cfg.try_deserialize()?)
    }
}
