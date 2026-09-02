#!/usr/bin/env python3
"""Fail if any benchmark regressed by more than a given percentage.

Reads `critcmp` output, which prints one row per benchmark with a column per
baseline and a ratio against the fastest. We compare the two named baselines
directly rather than trusting the ratio column, so the direction is explicit.

A benchmark present in one baseline and not the other is reported and ignored:
that is a benchmark being added or removed, not a regression.
"""

import re
import sys

# critcmp rows look like:
#   group/name    1.00  12.3±0.45ms   ...   1.07  13.2±0.51ms   ...
TIME = re.compile(r"([0-9.]+)±[0-9.]+(ns|µs|us|ms|s)\b")
UNITS = {"ns": 1e-9, "µs": 1e-6, "us": 1e-6, "ms": 1e-3, "s": 1.0}


def seconds(value: str, unit: str) -> float:
    return float(value) * UNITS[unit]


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: check_bench_regression.py <critcmp-output> <max-percent>")
        return 2

    path, limit = sys.argv[1], float(sys.argv[2])
    regressions, compared, skipped = [], 0, 0

    for line in open(path, encoding="utf-8"):
        times = TIME.findall(line)
        if len(times) != 2:
            # Header, separator rule, or a benchmark present in only one
            # baseline. Only the last of those is worth counting.
            stripped = line.strip()
            if stripped and not stripped.startswith(("group", "---")):
                skipped += 1
            continue

        name = line.split()[0]
        base = seconds(*times[0])
        pr = seconds(*times[1])
        if base <= 0:
            continue

        compared += 1
        change = (pr - base) / base * 100.0
        marker = "SLOWER" if change > 0 else "faster"
        print(f"  {name:<48} {change:+7.1f}%  {marker}")
        if change > limit:
            regressions.append((name, change))

    print(f"\ncompared {compared} benchmarks, {skipped} row(s) not comparable")

    if regressions:
        print(f"\nRegressions over {limit:.0f}%:")
        for name, change in regressions:
            print(f"  {name}: {change:+.1f}%")
        print(
            "\nIf this is a deliberate trade — correctness bought with time —"
            "\nsay so in the pull request and update the threshold in ci.yml"
            "\nwith the reason. Do not silence it quietly."
        )
        return 1

    print(f"\nNo benchmark regressed by more than {limit:.0f}%.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
