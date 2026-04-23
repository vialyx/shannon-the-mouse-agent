# shannon-the-mouse-agent

Cross-platform desktop agent that continuously captures OS-level mouse events
and computes a real-time **Shannon entropy risk score** based on movement
dynamics.  Telemetry is emitted as structured JSON to stdout and optionally
batched to a remote HTTP endpoint.

---

## Architecture

```
mouse-entropy-agent/
  src/
    main.rs       – tokio async entrypoint, event loop, graceful Ctrl-C shutdown
    capture.rs    – cross-platform OS mouse hook via rdev
    buffer.rs     – lock-free rolling window buffer (VecDeque)
    entropy.rs    – Shannon entropy, velocity jitter, risk-score computation
    scorer.rs     – threshold classification (LOW / MEDIUM / HIGH / CRITICAL)
    emitter.rs    – JSON stdout + optional HTTP POST with exponential backoff
    config.rs     – TOML / environment-variable configuration (config crate)
    lib.rs        – public module re-exports (used by integration tests)
```

## Algorithm

For every rolling window W (default 500 ms):

1. Compute delta vectors Δx, Δy and velocity v = √(Δx²+Δy²) / Δt
2. Compute direction angle θ = atan2(Δy, Δx) ∈ [0°, 360°)
3. Quantise θ into B bins (default 16, each 22.5°)
4. Compute Shannon entropy  H = −Σ p(b)·log₂(p(b))
5. Normalise:  H_norm = H / log₂(B)
6. Compute velocity jitter σ_v = std_dev(velocities)
7. risk_score = α·H_norm + β·normalise(σ_v)   (α=0.6, β=0.4)

| Score range | Level    | Interpretation                          |
|-------------|----------|-----------------------------------------|
| 0.0 – 0.3   | LOW      | Natural human movement                  |
| 0.3 – 0.6   | MEDIUM   | Unusual patterns / elevated jitter      |
| 0.6 – 0.8   | HIGH     | Robotic or scripted movement            |
| 0.8 – 1.0   | CRITICAL | Near-certain automation or attack       |

## Telemetry output (one JSON object per window)

```json
{
  "ts": 1714123456789,
  "window_ms": 500,
  "sample_count": 42,
  "entropy_raw": 3.61,
  "entropy_norm": 0.903,
  "velocity_mean": 128.4,
  "velocity_jitter": 34.2,
  "risk_score": 0.74,
  "risk_level": "HIGH",
  "session_id": "b1e2c3d4-..."
}
```

When `ANTHROPIC_API_KEY` is set and `risk_score` exceeds the critical threshold,
an `anomaly_explanation` key is added with a 1–2 sentence AI-generated
behavioural analysis.

---

## Prerequisites

### All platforms
- Rust stable toolchain: <https://rustup.rs>

### macOS
- Grant **Accessibility** permission to your terminal:  
  *System Settings → Privacy & Security → Accessibility → enable your app*

### Linux (X11)
Install development headers before building:

```bash
sudo apt-get install -y libxtst-dev libx11-dev libxcb1-dev libxfixes-dev pkg-config
```

The agent uses rdev with X11/XInput2.  Ensure `$DISPLAY` is set.  On Wayland,
XWayland must be running.

### Windows
No additional dependencies – rdev uses `SetWindowsHookEx` for user-space hooks.

---

## Build

```bash
# Clone
git clone https://github.com/vialyx/shannon-the-mouse-agent.git
cd shannon-the-mouse-agent

# Debug build
cargo build

# Release build (recommended for production)
cargo build --release

# Cross-compile for a specific target (example)
cargo build --release --target x86_64-unknown-linux-gnu
```

## Run

```bash
# Use defaults (reads config.toml if present)
./target/release/mouse-entropy-agent

# Override config via environment variables
MOUSE_AGENT__WINDOW__DURATION_MS=1000 \
MOUSE_AGENT__EMIT__HTTP_ENDPOINT=https://example.com/risk \
./target/release/mouse-entropy-agent
```

Press **Ctrl-C** for a clean shutdown.

---

## Configuration (`config.toml`)

```toml
[window]
duration_ms = 500   # ms per analysis window
bins = 16           # direction bins

[scoring]
alpha = 0.6         # entropy weight
beta  = 0.4         # jitter weight

[emit]
stdout = true
http_endpoint    = ""     # optional remote endpoint
http_interval_ms = 1000   # HTTP batch flush interval

[thresholds]
medium   = 0.3
high     = 0.6
critical = 0.8
```

All values can be overridden with environment variables using the
`MOUSE_AGENT__SECTION__KEY` convention (e.g.
`MOUSE_AGENT__WINDOW__BINS=32`).

---

## Tests

```bash
# Unit tests (entropy, scorer, buffer)
cargo test --lib

# Integration tests (1 000 synthetic events)
cargo test --test integration_test

# All tests
cargo test --workspace
```

---

## CI

GitHub Actions runs a build-and-test matrix on
**ubuntu-latest**, **macos-latest**, and **windows-latest** on every push and
pull-request to `main`.  See [`.github/workflows/ci.yml`](.github/workflows/ci.yml).

