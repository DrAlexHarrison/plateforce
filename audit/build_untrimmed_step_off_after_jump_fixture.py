"""Build the committed synthetic recording whose step off the plate follows the jump.

The result is committed and this script is how it is reproduced. Nothing here reads a corpus
and no athlete produced any sample of it.

The flight is not typed anywhere. The trace up to takeoff is built first, its net impulse over
system weight is integrated, and the flight lasts exactly the 2v/g that impulse buys, so jump
height from the impulse and jump height from the flight time agree on this recording rather than
one of them being asserted beside the other. The propulsive peak is solved for the stated takeoff
velocity by bisection for the same reason.

    python3 audit/build_untrimmed_step_off_after_jump_fixture.py

Pass --report to print what the trace holds without writing it.
"""

from __future__ import annotations

import math
import random
import sys
from pathlib import Path

FIXTURE = (
    Path(__file__).resolve().parent.parent
    / "crates/plateforce-conformance/fixtures/synthetic_untrimmed_step_off_after_jump.force.txt"
)

SAMPLE_RATE_HZ = 1200.0
GRAVITY_METERS_PER_SECOND_SQUARED = 9.80665
SYSTEM_WEIGHT_NEWTONS = 600.0
SYSTEM_MASS_KILOGRAMS = SYSTEM_WEIGHT_NEWTONS / GRAVITY_METERS_PER_SECOND_SQUARED

# The plate reads noisier under load than unloaded, as the sibling fixture does.
LOADED_NOISE_NEWTONS = 3.0
UNLOADED_NOISE_NEWTONS = 1.0

TAKEOFF_VELOCITY_METERS_PER_SECOND = 2.30
SEED = 20260803


def samples(seconds: float) -> int:
    return int(round(seconds * SAMPLE_RATE_HZ))


def raised_cosine(index: int, count: int) -> float:
    """A join carrying no corner, so a derivative rule reads a shape rather than a step."""
    return 0.5 - 0.5 * math.cos(math.pi * (index + 1) / count)


def noise_half_width_newtons(load_newtons: float) -> float:
    fraction = min(1.0, max(0.0, load_newtons / SYSTEM_WEIGHT_NEWTONS))
    return UNLOADED_NOISE_NEWTONS + (LOADED_NOISE_NEWTONS - UNLOADED_NOISE_NEWTONS) * fraction


def build(propulsive_peak_newtons: float) -> tuple[list[float], int, float, float]:
    generator = random.Random(SEED)
    force_newtons: list[float] = []

    def level(seconds: float, load_newtons: float) -> None:
        for _ in range(samples(seconds)):
            half_width = noise_half_width_newtons(load_newtons)
            force_newtons.append(load_newtons + generator.uniform(-half_width, half_width))

    def arc(seconds: float, start_newtons: float, end_newtons: float) -> None:
        count = samples(seconds)
        for index in range(count):
            value = start_newtons + (end_newtons - start_newtons) * raised_cosine(index, count)
            half_width = noise_half_width_newtons(value)
            force_newtons.append(value + generator.uniform(-half_width, half_width))

    # Quiet standing. The first second of it is the weighing window every rule reads.
    level(1.2, SYSTEM_WEIGHT_NEWTONS)
    # Unweighting, then braking and propulsion as one rise rather than two.
    arc(0.16, SYSTEM_WEIGHT_NEWTONS, 240.0)
    arc(0.14, 240.0, SYSTEM_WEIGHT_NEWTONS)
    arc(0.26, SYSTEM_WEIGHT_NEWTONS, propulsive_peak_newtons)
    arc(0.10, propulsive_peak_newtons, 0.0)

    takeoff_index = len(force_newtons)
    net_impulse_newton_seconds = (
        sum(value - SYSTEM_WEIGHT_NEWTONS for value in force_newtons) / SAMPLE_RATE_HZ
    )
    takeoff_velocity = net_impulse_newton_seconds / SYSTEM_MASS_KILOGRAMS
    flight_seconds = 2.0 * takeoff_velocity / GRAVITY_METERS_PER_SECOND_SQUARED

    level(flight_seconds, 0.0)
    # Landing, then the athlete settles back onto both feet.
    arc(0.03, 0.0, 4.0 * SYSTEM_WEIGHT_NEWTONS)
    arc(0.10, 4.0 * SYSTEM_WEIGHT_NEWTONS, 700.0)
    arc(0.25, 700.0, SYSTEM_WEIGHT_NEWTONS)
    level(0.8, SYSTEM_WEIGHT_NEWTONS)
    # The athlete steps off the plate and the recording keeps running, which is the whole
    # point of the trace: the emptiest the plate ever reads is here and not in the flight.
    arc(0.15, SYSTEM_WEIGHT_NEWTONS, 0.0)
    level(2.0, 0.0)
    return force_newtons, takeoff_index, takeoff_velocity, flight_seconds


def solve_propulsive_peak_newtons() -> float:
    low, high = SYSTEM_WEIGHT_NEWTONS, 6000.0
    for _ in range(60):
        middle = 0.5 * (low + high)
        if build(middle)[2] < TAKEOFF_VELOCITY_METERS_PER_SECOND:
            low = middle
        else:
            high = middle
    return 0.5 * (low + high)


def report(
    force_newtons: list[float],
    takeoff_index: int,
    takeoff_velocity: float,
    flight_seconds: float,
    propulsive_peak_newtons: float,
) -> None:
    velocity = 0.0
    minimum_velocity = 0.0
    for value in force_newtons[:takeoff_index]:
        velocity += (
            (value - SYSTEM_WEIGHT_NEWTONS) / SYSTEM_MASS_KILOGRAMS / SAMPLE_RATE_HZ
        )
        minimum_velocity = min(minimum_velocity, velocity)
    minimum_newtons = min(force_newtons)
    minimum_index = force_newtons.index(minimum_newtons)
    height_from_impulse = takeoff_velocity**2 / (2.0 * GRAVITY_METERS_PER_SECOND_SQUARED)
    height_from_flight = GRAVITY_METERS_PER_SECOND_SQUARED * flight_seconds**2 / 8.0
    print(f"samples {len(force_newtons)}, duration {len(force_newtons)/SAMPLE_RATE_HZ:.4f} s")
    print(
        f"propulsive peak {propulsive_peak_newtons:.1f} N "
        f"({propulsive_peak_newtons/SYSTEM_WEIGHT_NEWTONS:.2f} system weights), "
        f"landing peak {max(force_newtons):.1f} N"
    )
    print(f"takeoff sample {takeoff_index} at {takeoff_index/SAMPLE_RATE_HZ:.4f} s")
    print(f"countermovement velocity minimum {minimum_velocity:.4f} m/s")
    print(f"takeoff velocity {takeoff_velocity:.4f} m/s, flight {flight_seconds:.4f} s")
    print(
        f"jump height from the impulse {height_from_impulse*100:.2f} cm, "
        f"from the flight time {height_from_flight*100:.2f} cm"
    )
    print(
        f"lowest sample {minimum_newtons:.4f} N at {minimum_index} "
        f"({minimum_index/SAMPLE_RATE_HZ:.4f} s), which is after the landing"
    )


def main(argv: list[str]) -> int:
    propulsive_peak_newtons = solve_propulsive_peak_newtons()
    force_newtons, takeoff_index, takeoff_velocity, flight_seconds = build(
        propulsive_peak_newtons
    )
    report(
        force_newtons,
        takeoff_index,
        takeoff_velocity,
        flight_seconds,
        propulsive_peak_newtons,
    )
    if "--report" in argv:
        return 0
    FIXTURE.write_text("".join(f"{value:.4f}\n" for value in force_newtons))
    print(f"wrote {FIXTURE.name}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
