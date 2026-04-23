use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use mouse_entropy_agent::{
    buffer::{MouseSample, RollingBuffer},
    entropy::compute_risk,
};

fn build_samples(count: usize, pattern: &str) -> std::collections::VecDeque<MouseSample> {
    let mut buf = RollingBuffer::new(10_000);
    let base_ts = 1_700_000_000_000u64;

    for i in 0..count {
        let t = base_ts + (i as u64 * 5);
        let (x, y) = match pattern {
            "straight" => (i as f64, 100.0),
            "circular" => {
                let a = 2.0 * std::f64::consts::PI * (i as f64 / count.max(1) as f64);
                (500.0 + 100.0 * a.cos(), 500.0 + 100.0 * a.sin())
            }
            _ => {
                let a = 2.0 * std::f64::consts::PI * ((i % 16) as f64 / 16.0);
                let scale = 1.0 + i as f64 * 0.01;
                (
                    300.0 + 10.0 * a.cos() * scale,
                    300.0 + 10.0 * a.sin() * scale,
                )
            }
        };

        buf.push(MouseSample {
            x,
            y,
            timestamp_ms: t,
        });
    }

    buf.window_samples().clone()
}

fn bench_compute_risk(c: &mut Criterion) {
    let mut group = c.benchmark_group("compute_risk");
    let sizes = [100usize, 1_000usize, 10_000usize];

    for &size in &sizes {
        for pattern in ["straight", "circular", "multi_dir"] {
            let samples = build_samples(size, pattern);
            group.throughput(Throughput::Elements(size as u64));
            group.bench_with_input(BenchmarkId::new(pattern, size), &samples, |b, samples| {
                b.iter(|| {
                    let result = compute_risk(
                        black_box(samples),
                        black_box(16),
                        black_box(0.6),
                        black_box(0.4),
                    );
                    black_box(result);
                });
            });
        }
    }

    group.finish();
}

criterion_group!(benches, bench_compute_risk);
criterion_main!(benches);
