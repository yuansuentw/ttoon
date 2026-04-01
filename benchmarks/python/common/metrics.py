from __future__ import annotations

import gc
import statistics
import time
from collections.abc import Callable
from dataclasses import asdict, dataclass


@dataclass(frozen=True)
class BenchmarkStats:
    mean_ms: float
    median_ms: float
    min_ms: float
    max_ms: float
    stdev_ms: float

    def to_dict(self) -> dict[str, float]:
        return asdict(self)


def measure_sync(
    func: Callable[[], object],
    *,
    warmups: int,
    iterations: int,
    disable_gc: bool = False,
) -> tuple[list[float], BenchmarkStats]:
    if warmups < 0 or iterations <= 0:
        raise ValueError("warmups must be >= 0 and iterations must be > 0")

    gc_was_enabled = gc.isenabled()
    if disable_gc and gc_was_enabled:
        gc.disable()

    try:
        for _ in range(warmups):
            func()

        samples_ms: list[float] = []
        for _ in range(iterations):
            start_ns = time.perf_counter_ns()
            func()
            elapsed_ns = time.perf_counter_ns() - start_ns
            samples_ms.append(elapsed_ns / 1_000_000)
    finally:
        if disable_gc and gc_was_enabled:
            gc.enable()

    return samples_ms, summarize_samples(samples_ms)


def summarize_samples(samples_ms: list[float]) -> BenchmarkStats:
    if not samples_ms:
        raise ValueError("samples_ms must not be empty")

    stdev_ms = statistics.stdev(samples_ms) if len(samples_ms) > 1 else 0.0
    return BenchmarkStats(
        mean_ms=statistics.fmean(samples_ms),
        median_ms=statistics.median(samples_ms),
        min_ms=min(samples_ms),
        max_ms=max(samples_ms),
        stdev_ms=stdev_ms,
    )
