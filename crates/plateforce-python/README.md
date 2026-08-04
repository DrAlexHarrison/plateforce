# plateforce

Force-plate analysis where a result carries the method that produced it. You pick a
published method from the registry.

```
pip install plateforce
```

The machine that installs it needs no compiler and no Rust toolchain. One abi3 wheel per
platform covers Python 3.11 and every later version, and the method registry travels inside
the wheel, so the digest a result reports names the same bytes on every machine that
installed the same version.

**`plateforce` is not `forceplate`.** The similarly named CRAN package, by Hartmann, Koger
and Johannsen, analyses posturography: centre-of-pressure measures from quiet standing.
This one computes jump kinetics from a vertical ground reaction force trace. Neither is a
port of the other.

## Analysing one jump

```python
import numpy as np
import plateforce as pf

registry = pf.Registry.load()

force_newtons = np.loadtxt("trial.csv")        # vertical ground reaction force, newtons
trial = pf.Trial(force_newtons, sample_rate_hz=1200.0)

jump = pf.analyse_countermovement_jump(
    trial,
    weighing_epoch=registry.method("bwepoch.fixed_window").bind(duration=1.0),
    onset=registry.method("onset.threshold.noise_relative").bind(k=5.0),
    takeoff=registry.method("takeoff.threshold.absolute_force").bind(threshold_n=20.0),
)

print(jump.jump_height_takeoff_frame_meters.describe())
```

```
0.3419695652891413 meters
  jumpheight.takeoff.impulse_momentum {'gravity_meters_per_second_squared': 9.80665}
  registry declaring 2026-07-25 (content-1d554a5c548c0d9f)
    impulse.net_vertical.as_performance_determinant {'gravity_meters_per_second_squared': 9.80665}
      bwepoch.fixed_window {'duration': 1, 'start_seconds': 0}
        centre = mean
        dispersion = sample
      onset.threshold.noise_relative {'k': 5}
        degenerate_band = refuse
        reference_distribution = quiet_stance_force
        sd_convention = sample
        onset.op.backward_offset_fixed {'offset_ms': 30}
        onset.op.crossing_selection {}
          selection = first
        onset.op.direction {}
          direction = below_only
        onset.op.persistence {'span_ms': 0}
        onset.op.search_floor_at_weighing_epoch_end {'floor_seconds': 1}
        bwepoch.fixed_window {'duration': 1}
          central_tendency = mean
          dispersion_estimator = sample
      takeoff.threshold.absolute_force {'threshold_n': 20}
        residual_comparison = signed_value
        bwepoch.fixed_window {'duration': 1}
          central_tendency = mean
          dispersion_estimator = sample
  acquisition block incomplete, so this result cannot be declared to match another lab's
```

Everything under the number is what a second lab needs to reproduce it. The quiet-standing
window appears three times because it moved the answer three times: as system weight, as
the noise scale that placed onset, and through the impulse that produced the velocity.

## Why the number carries all that

Two independent open-source implementations of the same published methods, run over the
same 244 countermovement jumps, agree at r = 0.961 on jump height and r = 0.696 on time to
takeoff. Both were tested. Both passed their own tests.

Across those trials, the spread between published methods within a single trial is a median
3.51 cm of jump height. The training intervention that dataset was collected to measure
moved jump height by 1.98 cm.

A float cannot tell you which of nine published onset rules produced it, so two labs
comparing floats have no way to find out they were never measuring the same thing.

## A result is not a float

```python
>>> jump.jump_height_takeoff_frame_meters
Measured(value=0.3419695652891413, unit='meters', method_id='jumpheight.takeoff.impulse_momentum')

>>> jump.jump_height_takeoff_frame_meters + 0.01
TypeError: unsupported operand type(s) for +: 'plateforce.Measured' and 'float'
```

The bare number is one attribute away, and asking for it is a visible act:

```python
>>> jump.jump_height_takeoff_frame_meters.value
0.3419695652891413
```

Each result also carries `.unit` and `.provenance`. To find the parameter that moved a
downstream number, ask the chain rather than walking it:

```python
>>> jump.time_to_takeoff_seconds.provenance.parameters_of("onset.threshold.noise_relative")
{'k': 5.0}
```

## Look before you choose

Some published rules do not disagree, they find the wrong event. Two onset rules in this
corpus place the start of the movement more than two seconds before takeoff on roughly one
trial in seven, on a movement lasting under a second, and their median behaviour looks
ordinary while they do it.

```python
>>> for entry in registry.methods_that_can_fail():
...     print(entry)
MethodEntry('onset.op.backward_offset_fixed', status='accepted', implemented=False, FAILS on 36 of 241 trials (14.9%, silent))
```

`silent` means nothing warns you, so the entry shows the failure rate before you can bind
it.

An entry also carries `.rule`, `.citations`, `.biases`, `.parameters` and `.gui.surfacing`,
the registry's own ruling on how hard an interface should push the choice at a user. A bias
always states the `criterion` it was measured against, because a bias figure without one
cannot be added to anything safely.

## Errors name the method and the parameter

```python
>>> pf.analyse_countermovement_jump(quiet_standing_trial, weighing_epoch, onset, takeoff)
NoCrossingError: onset.threshold.noise_relative(k = 5) found no crossing within the search bound of 2.5 s
```

The fields are on the exception, so a batch run can branch on them instead of parsing the
sentence:

```python
>>> error.method_id, error.parameter, error.value
('onset.threshold.noise_relative', 'k', 5.0)
```

The registry describes the literature, which is larger than the set of rules any one piece
of software runs. Selecting an entry with no rule behind it fails by name rather than
resolving to something near it:

```python
MethodNotImplementedError: 'onset.op.backward_offset_fixed' was passed as
the onset method, and 'onset.threshold.noise_relative' is the rule available for that step.
Available: ["bwepoch.fixed_window", "onset.threshold.noise_relative",
"takeoff.threshold.absolute_force"]
```

Check first with `registry.method(id).implemented`.

## Missing data is reported, never inferred

Vendor exports write `0`, `-1` or `9999` to mean "no measurement". Reading one as a real
value moved a published correlation in this corpus by 0.16. Declare the convention your
export uses and the trial reports what matched it:

```python
>>> trial = pf.Trial(force_newtons, sample_rate_hz=1200.0, sentinel=pf.Sentinel.zero())
>>> trial.exclusions
Exclusions(dropped_samples=600, sentinel_convention='zero', reason='600 sample(s) reported and kept in place: removing them would shift the time base')
```

Samples are counted and reported, never removed: closing a gap in a trace would shift every
timestamp after it. For a column of per-trial results, where dropping a row is the right
thing, use `pf.partition_sentinel_values(values, pf.Sentinel.zero())`.

## Saying whether two results match

Matching analysis is not enough to make two numbers comparable. A 50 ms contact debounce
living in one plate's firmware changes the answer and no reanalysis recovers from not
knowing it, so acquisition is part of the record:

```python
trial = pf.Trial(
    force_newtons,
    sample_rate_hz=1200.0,
    acquisition=pf.Acquisition(
        filter_at_capture="none",
        tare_state="tared_before_trial",
        plate_natural_frequency_hz=800.0,
        floor_surface="concrete",
        firmware_version="2.4.1",
    ),
)
```

Until every member is present, `provenance.acquisition_complete` stays `False` and every
result says so. `pf.Acquisition(...).missing` lists what is still needed.

## Arrays

`Trial` reads any float64 buffer without a Python-level loop: a numpy array, an
`array.array('d')`, a memoryview, and non-contiguous views such as `column[::2]`. Lists and
tuples work too and convert element by element.

A narrower array type is refused rather than widened, because a float32 trace does not carry
the precision the impulse identity is checked at:

```python
>>> pf.Trial(force.astype(np.float32), sample_rate_hz=1200.0)
TrialError: force_newtons has dtype float32 and plateforce reads float64. Convert it with
.astype('float64') so the widening is recorded as your choice
```

numpy is not a dependency of this package.

## Two fields worth reading

`jump.unregistered_methods` lists the steps that ran with no registry entry describing them.
They are choices that moved the number, printed rather than left to be discovered.

`jump.weighing_epoch_tied_window_count` is 1 for a fixed window. Above 1 means a search rule
found exact ties and did not identify a single window, so anything downstream treating the
selection as determined is reading an artefact of the arithmetic.

## Licence

Apache-2.0.
