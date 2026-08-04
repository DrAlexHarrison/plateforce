"""Audit of the numbers on the plateforce front page against the matrix they come from.

Run:
    python audit/headline_audit.py

Requires rule_matrix.csv alongside this file. That matrix is derived from a
re-identifiable 2011 corpus whose consent position is unresolved, so it is not
released with this repository.

Reads rule_matrix.csv (244 trials by 56 method variants) and recomputes the spreads, the
failure rates and the sentinel effects the README quotes, plus the ordinary least products
regression that Dos'Santos, Lake and Comfort 2022 argue for in place of a correlation when
neither implementation is a gold standard. Section 1 tests the README's time to takeoff
correlation against every pair the matrix holds. The jump height correlation beside it is
quoted from a pairwise run over the two open implementations, and the 1.98 cm intervention
effect from the study the corpus was collected for.
"""
import itertools
import re
from pathlib import Path

import numpy as np
import pandas as pd

CSV = Path(__file__).parent / "rule_matrix.csv"
README = Path(__file__).resolve().parent.parent / "README.md"
RNG = np.random.default_rng(20260725)
BOOTSTRAP_RESAMPLES = 5000
BLOWUP_SECONDS = 2.0
# Wide enough that a pair rounding to the quoted figure counts as reproducing it.
REPRODUCED_WITHIN = 0.02


def claimed_time_to_takeoff_correlation():
    """The figure the front page states, read off the page rather than copied from it."""
    matched = re.search(
        r"r = (\d+\.\d+) on time to takeoff", README.read_text(encoding="utf-8")
    )
    if not matched:
        raise SystemExit(
            f"{README} states no time to takeoff correlation, so there is nothing to audit"
        )
    return float(matched.group(1))


df = pd.read_csv(CSV)
time_to_takeoff_columns = sorted(c for c in df.columns if c.startswith("ttt_"))
jump_height_columns = sorted(
    c for c in df.columns if c.startswith("jh_") and c.endswith("_cm")
)
# AccuPower is the commercial reference instrument, not a published analysis method.
published_jump_height_columns = [
    c for c in jump_height_columns if c != "jh_accupower_cm"
]


def rule(title):
    print("\n" + "=" * 78)
    print(title)
    print("=" * 78)


def pearson(a, b, extra_mask=None):
    x, y = df[a].to_numpy(float), df[b].to_numpy(float)
    m = np.isfinite(x) & np.isfinite(y)
    if extra_mask is not None:
        m &= extra_mask
    return np.corrcoef(x[m], y[m])[0, 1], int(m.sum())


def ordinary_least_products(x, y):
    """Geometric mean regression. Slope is sd(y)/sd(x) carrying the sign of r."""
    m = np.isfinite(x) & np.isfinite(y)
    x, y = x[m], y[m]
    r = np.corrcoef(x, y)[0, 1]
    slope = np.sign(r) * np.std(y, ddof=1) / np.std(x, ddof=1)
    return r, slope, np.mean(y) - slope * np.mean(x), len(x)


def olp_with_intervals(a, b, unit, extra_mask=None):
    x, y = df[a].to_numpy(float), df[b].to_numpy(float)
    m = np.isfinite(x) & np.isfinite(y)
    if extra_mask is not None:
        m &= extra_mask
    x, y = x[m], y[m]
    r, slope, intercept, n = ordinary_least_products(x, y)
    slopes, intercepts = [], []
    for _ in range(BOOTSTRAP_RESAMPLES):
        i = RNG.integers(0, n, n)
        _, s, b_, _ = ordinary_least_products(x[i], y[i])
        slopes.append(s)
        intercepts.append(b_)
    slo, shi = np.percentile(slopes, [2.5, 97.5])
    ilo, ihi = np.percentile(intercepts, [2.5, 97.5])
    proportional = not (slo <= 1.0 <= shi)
    fixed = not (ilo <= 0.0 <= ihi)
    print(f"\n   {a}  ->  {b}   n = {n}")
    print(f"      Pearson r      {r:+.4f}")
    print(f"      OLP slope      {slope:.4f}  95% CI [{slo:.4f}, {shi:.4f}]  "
          f"{'PROPORTIONAL bias' if proportional else 'no proportional bias'}")
    print(f"      OLP intercept  {intercept:+.4f} {unit}  95% CI [{ilo:+.4f}, {ihi:+.4f}]  "
          f"{'FIXED offset' if fixed else 'no fixed offset'}")


claimed = claimed_time_to_takeoff_correlation()

rule(f"1.  CAN THE README'S r = {claimed} BE REPRODUCED?  all pairs of the 10 ttt columns")
pairs = []
for a, b in itertools.combinations(time_to_takeoff_columns, 2):
    r, n = pearson(a, b)
    pairs.append((r, a, b, n))
pairs.sort()
reproducing = 0
for r, a, b, n in pairs:
    within = abs(r - claimed) < REPRODUCED_WITHIN
    reproducing += within
    flag = f"   <== within {REPRODUCED_WITHIN} of the README's {claimed}" if within else ""
    print(f"   r = {r:+.4f}  n={n:>3}   {a:<22} vs {b:<22}{flag}")
print(
    f"\n   pairs examined: {len(pairs)}.  {reproducing} of them land within "
    f"{REPRODUCED_WITHIN} of {claimed}."
)

rule("2.  THE 38 PERCENT AND 3.51 cm SPREADS, WITH THEIR DENOMINATORS")
for label, cols, unit in (
    ("time to takeoff", time_to_takeoff_columns, "s"),
    ("jump height, published methods only", published_jump_height_columns, "cm"),
    ("jump height, including the commercial reference", jump_height_columns, "cm"),
):
    M = df[cols].to_numpy(float)
    spread = np.nanmax(M, axis=1) - np.nanmin(M, axis=1)
    median = np.nanmedian(M, axis=1)
    print(f"\n   {label}  ({len(cols)} columns, {len(df)} trials)")
    print(f"      median within-trial spread  {np.nanmedian(spread):.4f} {unit}")
    print(f"      as percent of trial median  {np.nanmedian(100 * spread / median):.1f}%")

rule("3.  MISSING-DATA SENTINELS IN THE COMMERCIAL REFERENCE")
zero_jump_height = df["jh_accupower_cm"].to_numpy(float) == 0
print(f"\n   AccuPower reports exactly 0.00 on {int(zero_jump_height.sum())} of {len(df)} trials,")
print("   simultaneously for jump height, time to takeoff and RSI.")
print("   On those same trials the open implementations report:")
for c in ("jh_jm_cm", "jh_sams_bw_cm", "jh_chronojump_cm"):
    v = df.loc[zero_jump_height, c].to_numpy(float)
    v = v[np.isfinite(v)]
    if len(v):
        print(f"      {c:<20} median {np.median(v):>7.2f} cm, range {v.min():.2f} to {v.max():.2f} cm")
print("\n   A 0.00 cm countermovement jump is not physically possible. These are missing values.")
print(f"\n   {'comparison':<44}{'as filed':>18}{'sentinels removed':>22}")
for c in published_jump_height_columns:
    r1, n1 = pearson(c, "jh_accupower_cm")
    r2, n2 = pearson(c, "jh_accupower_cm", ~zero_jump_height)
    print(f"   {c + ' vs accupower':<44}{f'r={r1:+.4f} n={n1}':>18}{f'r={r2:+.4f} n={n2}':>22}")

rule("4.  THE DISAGREEMENT IS A FAILURE RATE, NOT A BIAS")
print(f"\n   {'onset rule':<24}{'n > 2 s':>9}{'% of trials':>13}{'median':>10}{'p95':>10}{'max':>10}")
for c in time_to_takeoff_columns:
    v = df[c].to_numpy(float)
    v = v[np.isfinite(v)]
    n_blow = int((v > BLOWUP_SECONDS).sum())
    print(f"   {c:<24}{n_blow:>9}{100 * n_blow / len(v):>12.1f}%"
          f"{np.median(v):>10.3f}{np.percentile(v, 95):>10.3f}{v.max():>10.3f}")
print("\n   What the blow-ups do to the pairwise correlation:")
for a, b in (("ttt_jm", "ttt_bilalovski"), ("ttt_jm", "ttt_sd_minus_30ms")):
    x, y = df[a].to_numpy(float), df[b].to_numpy(float)
    healthy = (x < BLOWUP_SECONDS) & (y < BLOWUP_SECONDS)
    r1, n1 = pearson(a, b)
    r2, n2 = pearson(a, b, healthy)
    print(f"      {a} vs {b}")
    print(f"         all trials         n={n1:>3}  r = {r1:+.4f}")
    print(f"         blow-ups excluded  n={n2:>3}  r = {r2:+.4f}")

rule("5.  ORDINARY LEAST PRODUCTS.  fixed offset, proportional, or both")
print("\n--- time to takeoff, open implementation against open implementation ---")
for a, b in (("ttt_jm", "ttt_sams_bw"), ("ttt_jm", "ttt_chronojump")):
    olp_with_intervals(a, b, "s")
print("\n--- jump height against the commercial reference, sentinels removed ---")
for a in ("jh_jm_cm", "jh_sams_bw_cm", "jh_chronojump_cm"):
    olp_with_intervals(a, "jh_accupower_cm", "cm", ~zero_jump_height)
