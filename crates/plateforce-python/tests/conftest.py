"""Fixtures for the binding tests.

The registry used here is written by the tests rather than read from `registry/`, so a
test failure means the binding changed and never that the registry gained an entry. One
test in `test_registry.py` reads the shipped registry on purpose, to catch an id drifting
out from under the analysis dispatch.

All force data is synthetic. No trial from the corpus appears in this repository.
"""

import textwrap

import numpy as np
import pytest

SAMPLE_RATE_HZ = 1200.0
SYSTEM_MASS_KILOGRAMS = 60.0
STANDARD_GRAVITY = 9.80665

CONSTRUCTS = """
[[construct]]
id = "system_weight"
title = "Weight of athlete plus any external load during the weighing epoch"
unit = "newtons"

[[construct]]
id = "movement_onset"
title = "The instant the movement begins"
unit = "seconds"

[[construct]]
id = "takeoff"
title = "The instant the last foot leaves the plate"
unit = "seconds"

[[construct]]
id = "analysis_window"
title = "The stretch of the recording a number is taken over"
unit = "seconds"

[[construct]]
id = "peak_force"
title = "The biggest force"
unit = "newtons"
"""

METHODS = """
[[method]]
id = "bwepoch.fixed_window"
construct = "system_weight"
title = "Mean of force over a fixed span at a fixed anchor"
rule = "System weight is the mean of force over a fixed span anchored at the recording start."
status = "accepted"
confidence = "high"

[[method.parameter]]
name = "duration"
unit = "seconds"
published_values = [0.5, 1.0, 2.0]
default = 1.0
default_source = "owen2014"
required = true

[[method.citation]]
key = "owen2014"
role = "proposes"
reference = "Owen et al. 2014, JSCR 28:1552-1558"
obtained = true

[[method]]
id = "onset.threshold.noise_relative"
construct = "movement_onset"
title = "Noise-relative threshold, k standard deviations of the reference epoch"
rule = "Onset is the first sample departing the quiet mean by k standard deviations."
status = "accepted"
confidence = "high"
debate = "genuine"

[[method.parameter]]
name = "k"
unit = "standard_deviations"
published_values = [2.0, 3.0, 5.0, 10.0]
default = 5.0
default_source = "owen2014"
required = true

[[method.parameter]]
name = "sd_convention"
unit = "enumeration"
notes = "population or sample."

[[method.citation]]
key = "owen2014"
role = "proposes"
reference = "Owen et al. 2014, JSCR 28:1552-1558"
obtained = true

[[method.bias]]
magnitude = 0.026
unit = "seconds"
direction = "late"
criterion = "onset.manual_visual.tillin2010"
criterion_kind = "human_visual"
conditional_on_success = true

[[method]]
id = "takeoff.threshold.absolute_force"
construct = "takeoff"
title = "First sample past a fixed newton threshold"
rule = "Takeoff is the first sample below a fixed newton threshold."
status = "accepted"
confidence = "high"

[[method.parameter]]
name = "threshold_n"
unit = "newtons"
published_values = [10.0, 20.0, 30.0]
default = 20.0
default_source = "vald_glossary"
required = true

[[method.parameter]]
name = "persistence_ms"
unit = "milliseconds"
published_values = [15.0, 30.0, 250.0]

[[method.citation]]
key = "vald_glossary"
role = "proposes"
reference = "VALD ForceDecks Technical Glossary V2.0"
obtained = true

# Present so a test can assert the failure block reaches Python. Rate matches 36/241.
[[method]]
id = "onset.threshold.noise_relative_forward_offset"
construct = "movement_onset"
title = "Noise-relative threshold with the offset applied forward"
rule = "As the noise-relative threshold, with the offset applied in the direction of travel."
status = "legacy"
confidence = "high"
debate = "vendor_or_legacy"

[[method.parameter]]
name = "k"
unit = "standard_deviations"
published_values = [5.0]
default = 5.0
default_source = "owen2014"
required = true

[method.failure]
rate = 0.1494
numerator = 36
denominator = 241
corpus = "synthetic_fixture"
definition = "time to takeoff exceeding 2.0 s on a movement whose true duration is under 1 s"
detectability = "silent"

[method.gui]
surfacing = "force_a_decision"
sensitivity = "high"

[[method]]
id = "window_end.takeoff.detected"
construct = "analysis_window"
title = "The recording up to the sample takeoff was placed at"
rule = "The analysis window runs from the first sample of the recording to the sample the bound takeoff rule placed."
status = "accepted"
confidence = "high"

[[method]]
id = "force.peak.gross"
construct = "peak_force"
title = "The biggest force, system weight included"
rule = "Peak force is the maximum of the force series with system weight included in the value."
status = "accepted"
confidence = "high"

[[method]]
id = "force.peak.estimator"
construct = "peak_force"
title = "The biggest force, read off a centred average of stated width"
rule = "Peak force is the maximum of the raw series, or the maximum of a centred moving average of stated width."
status = "accepted"
confidence = "high"

[[method.parameter]]
name = "averaging_window_seconds"
unit = "seconds"
default = 0.0
default_source = "synthetic_fixture"
"""


@pytest.fixture(scope="session")
def registry_path(tmp_path_factory):
    root = tmp_path_factory.mktemp("registry")
    (root / "constructs.toml").write_text(textwrap.dedent(CONSTRUCTS))
    (root / "methods").mkdir()
    (root / "methods" / "seed.toml").write_text(textwrap.dedent(METHODS))
    return root


@pytest.fixture(scope="session")
def registry(registry_path):
    import plateforce as pf

    return pf.Registry.load(registry_path, version="fixture-1")


@pytest.fixture
def force_newtons():
    """Quiet standing with noise, an unloading ramp, a push, then flight.

    Shaped so every rule under test finds its event: the quiet window has a non-zero
    standard deviation for the noise-relative band, and the flight phase is long enough to
    clear a 30 ms persistence span.
    """
    generator = np.random.default_rng(11)
    weight = SYSTEM_MASS_KILOGRAMS * STANDARD_GRAVITY
    quiet = weight + generator.normal(0.0, 1.0, int(1.0 * SAMPLE_RATE_HZ))
    unloading = np.linspace(weight, 380.0, int(0.25 * SAMPLE_RATE_HZ))
    push = np.linspace(380.0, 1500.0, int(0.25 * SAMPLE_RATE_HZ))
    flight = np.zeros(int(0.5 * SAMPLE_RATE_HZ))
    return np.concatenate([quiet, unloading, push, flight])


@pytest.fixture
def trial(force_newtons):
    import plateforce as pf

    return pf.Trial(force_newtons, sample_rate_hz=SAMPLE_RATE_HZ)


@pytest.fixture
def complete_acquisition():
    import plateforce as pf

    return pf.Acquisition(
        filter_at_capture="none",
        tare_state="tared_before_trial",
        plate_natural_frequency_hz=800.0,
        floor_surface="concrete",
        firmware_version="synthetic-0",
    )


@pytest.fixture
def bound_methods(registry):
    return (
        registry.method("bwepoch.fixed_window").bind(duration=1.0),
        registry.method("onset.threshold.noise_relative").bind(k=5.0),
        registry.method("takeoff.threshold.absolute_force").bind(
            threshold_n=20.0, persistence_ms=30.0
        ),
    )
