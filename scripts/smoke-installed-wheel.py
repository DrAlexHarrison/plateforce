"""Analyse a trial using only an installed wheel and the shipped registry.

Nothing here imports from the repository, so a pass means somebody who ran
`pip install plateforce` gets a working analysis with no compiler and no Rust toolchain.
Takes the path to `registry/` as its one argument.
"""

import sys

import numpy as np

import plateforce as pf

SAMPLE_RATE_HZ = 1200.0
STANDARD_GRAVITY = 9.80665
SYSTEM_MASS_KILOGRAMS = 60.0

registry = pf.Registry.load(sys.argv[1], version="smoke")
census = registry.census
print(
    f"{census.constructs} constructs, {census.computation_entries} computation entries, "
    f"{census.protocol_entries} protocol entries"
)
assert census.constructs > 0
assert census.computation_entries > 0

# Quiet standing, an unloading ramp, a push, then flight. Synthetic, so no subject data
# reaches a public runner.
generator = np.random.default_rng(11)
system_weight_newtons = SYSTEM_MASS_KILOGRAMS * STANDARD_GRAVITY
vertical_ground_reaction_force_newtons = np.concatenate(
    [
        system_weight_newtons + generator.normal(0.0, 1.0, int(1.0 * SAMPLE_RATE_HZ)),
        np.linspace(system_weight_newtons, 380.0, int(0.25 * SAMPLE_RATE_HZ)),
        np.linspace(380.0, 1500.0, int(0.25 * SAMPLE_RATE_HZ)),
        np.zeros(int(0.5 * SAMPLE_RATE_HZ)),
    ]
)

trial = pf.Trial(vertical_ground_reaction_force_newtons, sample_rate_hz=SAMPLE_RATE_HZ)
jump = pf.analyse_countermovement_jump(
    trial,
    registry.method("bwepoch.fixed_window").bind(duration=1.0),
    registry.method("onset.threshold.noise_relative").bind(k=5.0),
    registry.method("takeoff.threshold.absolute_force").bind(
        threshold_n=20.0, persistence_ms=30.0
    ),
)

jump_height_centimeters = jump.jump_height_takeoff_frame_meters.value * 100.0
print(f"jump height {jump_height_centimeters:.2f} cm")
print(f"time to takeoff {jump.time_to_takeoff_seconds.value:.3f} s")
assert 1.0 < jump_height_centimeters < 200.0, jump_height_centimeters

# The result carrying its method is the whole product, so its absence fails the release.
provenance = jump.jump_height_takeoff_frame_meters.provenance
print(f"provenance {provenance!r}")
assert provenance is not None
assert provenance.method_id
