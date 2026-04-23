use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

/// A single telemetry window emitted as JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    /// Unix timestamp in milliseconds when the window was evaluated.
    pub ts: u64,
    pub window_ms: u64,
    pub sample_count: usize,
    pub entropy_raw: f64,
    pub entropy_norm: f64,
    pub velocity_mean: f64,
    pub velocity_jitter: f64,
    pub risk_score: f64,
    pub risk_level: String,
    pub session_id: String,
    /// Optional AI-generated explanation (present only when ANTHROPIC_API_KEY
    /// is set and the score exceeds the critical threshold).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anomaly_explanation: Option<String>,
}

/// Batch payload sent to the optional HTTP endpoint.
#[derive(Debug, Serialize)]
struct HttpBatch<'a> {
    agent_version: &'static str,
    os: &'static str,
    session_id: &'a str,
    events: &'a [TelemetryEvent],
}

/// Handles telemetry emission: stdout JSON and optional HTTP batching.
pub struct Emitter {
    pub stdout: bool,
    http_client: Option<Client>,
    http_endpoint: String,
    http_interval_ms: u64,
    session_id: String,
    batch_buffer: Vec<TelemetryEvent>,
    last_http_flush: Instant,
}

impl Emitter {
    pub fn new(
        stdout: bool,
        http_endpoint: String,
        http_interval_ms: u64,
        session_id: String,
    ) -> Self {
        let http_client = if http_endpoint.is_empty() {
            None
        } else {
            Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .ok()
        };

        Self {
            stdout,
            http_client,
            http_endpoint,
            http_interval_ms,
            session_id,
            batch_buffer: Vec::new(),
            last_http_flush: Instant::now(),
        }
    }

    /// Emit a telemetry event to stdout and/or the HTTP batch buffer.
    pub async fn emit(&mut self, event: &TelemetryEvent) -> Result<()> {
        if self.stdout {
            println!("{}", serde_json::to_string(event)?);
        }

        if self.http_client.is_some() {
            self.batch_buffer.push(event.clone());

            if self.last_http_flush.elapsed().as_millis() as u64 >= self.http_interval_ms {
                self.flush_http_batch().await;
                self.last_http_flush = Instant::now();
            }
        }

        Ok(())
    }

    /// Drain the batch buffer and POST to the configured endpoint with
    /// exponential backoff (up to 3 retries, capped at 30 s per retry).
    async fn flush_http_batch(&mut self) {
        if self.batch_buffer.is_empty() {
            return;
        }

        let client = match &self.http_client {
            Some(c) => c,
            None => return,
        };

        let batch = HttpBatch {
            agent_version: env!("CARGO_PKG_VERSION"),
            os: std::env::consts::OS,
            session_id: &self.session_id,
            events: &self.batch_buffer,
        };

        let body = match serde_json::to_string(&batch) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("HTTP batch serialisation error: {}", e);
                return;
            }
        };

        let mut delay_secs: u64 = 1;
        for attempt in 0..4u32 {
            let result = client
                .post(&self.http_endpoint)
                .header("Content-Type", "application/json")
                .body(body.clone())
                .send()
                .await;

            match result {
                Ok(resp) if resp.status().is_success() => {
                    self.batch_buffer.clear();
                    return;
                }
                Ok(resp) => {
                    eprintln!(
                        "HTTP POST returned {} (attempt {}/4)",
                        resp.status(),
                        attempt + 1
                    );
                }
                Err(e) => {
                    eprintln!("HTTP POST error (attempt {}/4): {}", attempt + 1, e);
                }
            }

            if attempt < 3 {
                sleep(Duration::from_secs(delay_secs.min(30))).await;
                delay_secs = (delay_secs * 2).min(30);
            }
        }

        // After all retries, discard the batch to avoid unbounded memory growth.
        eprintln!("HTTP batch dropped after 4 failed attempts.");
        self.batch_buffer.clear();
    }
}

/// Return the current Unix timestamp in milliseconds.
pub fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Call the Anthropic Claude API and return a 1–2 sentence behavioural
/// analysis of the recent telemetry windows.
///
/// Returns `None` (and logs a warning) on any error so the agent continues
/// operating normally without an AI explanation.
pub async fn get_anomaly_explanation(
    api_key: &str,
    recent_windows: &[TelemetryEvent],
) -> Result<String> {
    let client = Client::builder().timeout(Duration::from_secs(15)).build()?;

    let telemetry_json = serde_json::to_string(recent_windows)?;

    let request_body = serde_json::json!({
        "model": "claude-3-haiku-20240307",
        "max_tokens": 200,
        "system": "You are a behavioral biometrics analyst. Given mouse entropy telemetry, \
                   explain in 1-2 sentences why this session is flagged as high-risk and \
                   what type of automation or attack it may indicate.",
        "messages": [
            {
                "role": "user",
                "content": format!(
                    "Analyse this mouse telemetry and explain the risk:\n{}",
                    telemetry_json
                )
            }
        ]
    });

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&request_body)?)
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("Anthropic API returned {}", resp.status());
    }

    let body: serde_json::Value = resp.json().await?;
    let explanation = body["content"][0]["text"]
        .as_str()
        .unwrap_or("No explanation available.")
        .to_owned();

    Ok(explanation)
}
