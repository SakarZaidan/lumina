#!/usr/bin/env python3
"""Fail if any benchmark regressed by more than a given percentage.

Reads `critcmp` output, which prints one row per benchmark with a column per
baseline and a ratio against the fastest. We compare the two named baselines
directly rather than trusting the ratio column, so the direction is explicit.

A benchmark present in one baseline and not the other is reported and ignored:
that is a benchmark being added or removed, not a regression.

# Corroboration

One run on a shared runner is a noisy measurement, and the threshold alone
cannot separate a real regression from a bad neighbour on the host: this gate
once failed a change with `timeline_eval/2000 +52.9%` where the function that
benchmark exercises was byte-identical to the base branch.

So a regression must *corroborate* across the sizes of its own benchmark
family. Real slowdowns live in shared code and show up at every size, in
proportion; a single size moving alone while its siblings sit still is the
runner, not the patch. A family with one member has nothing to corroborate
against and is judged on the threshold alone.

An uncorroborated regression is still printed, and printed loudly. The point is
to stop the gate crying wolf, because a gate that fails on noise is a gate
somebody eventually switches off.
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
    changes, over, compared, skipped = [], [], 0, 0

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
        changes.append((name, change))
        if change > limit:
            over.append((name, change))

    print(f"\ncompared {compared} benchmarks, {skipped} row(s) not comparable")

    # Group by benchmark family — everything before the first `/`, which is the
    # criterion group name and therefore the same code at different sizes.
    families = {}
    for name, change in changes:
        families.setdefault(name.split("/")[0], []).append(change)

    # Half the threshold: a genuine regression in shared code shows at the other
    # sizes too, smaller but present. Noise does not.
    corroborate = limit / 2.0
    regressions, unconfirmed = [], []
    for name, change in over:
        siblings = families[name.split("/")[0]]
        if len(siblings) == 1 or sum(1 for c in siblings if c > corroborate) >= 2:
            regressions.append((name, change))
        else:
            unconfirmed.append((name, change))

    if unconfirmed:
        print(f"\nOver {limit:.0f}% but not corroborated by their own family:")
        for name, change in unconfirmed:
            siblings = ", ".join(f"{c:+.1f}%" for c in families[name.split("/")[0]])
            print(f"  {name}: {change:+.1f}%  (family: {siblings})")
        print(
            "  Treated as runner noise: a regression in shared code moves every"
            "\n  size of its family, not one. Read them anyway."
        )

    if regressions:
        print(f"\nRegressions over {limit:.0f}%, corroborated:")
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
