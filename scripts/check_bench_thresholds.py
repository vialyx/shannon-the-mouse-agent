#!/usr/bin/env python3
"""Fail CI when benchmark medians exceed configured thresholds."""

from __future__ import annotations

import json
import sys
from pathlib import Path

# Thresholds are in nanoseconds (median point estimate).
#
# These are tuned for GitHub-hosted shared runners, which are materially slower
# and noisier than local developer machines. Keep enough headroom to avoid
# flaky false positives while still catching clear regressions.
THRESHOLDS_NS = {
    "compute_risk/straight/1000": 25_000,
    "compute_risk/circular/1000": 45_000,
    "compute_risk/multi_dir/10000": 90_000,
}


def read_median_ns(criterion_root: Path, benchmark_path: str) -> float:
    estimates = criterion_root / benchmark_path / "new" / "estimates.json"
    if not estimates.exists():
        raise FileNotFoundError(f"missing benchmark estimates file: {estimates}")

    with estimates.open("r", encoding="utf-8") as f:
        payload = json.load(f)

    median = payload.get("median", {}).get("point_estimate")
    if median is None:
        raise ValueError(f"missing median.point_estimate in: {estimates}")

    return float(median)


def main() -> int:
    criterion_root = Path("target") / "criterion"

    failures: list[str] = []
    print("Benchmark regression guard (median ns):")

    for bench_path, threshold_ns in THRESHOLDS_NS.items():
        try:
            observed_ns = read_median_ns(criterion_root, bench_path)
        except (FileNotFoundError, ValueError, json.JSONDecodeError) as err:
            failures.append(str(err))
            continue

        status = "OK" if observed_ns <= threshold_ns else "FAIL"
        print(
            f"- {bench_path}: {observed_ns:.0f} ns "
            f"(threshold {threshold_ns} ns) [{status}]"
        )

        if observed_ns > threshold_ns:
            failures.append(
                f"{bench_path} exceeded threshold: {observed_ns:.0f} ns > {threshold_ns} ns"
            )

    if failures:
        print("\nPerformance regression guard failed:")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    print("\nPerformance regression guard passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
