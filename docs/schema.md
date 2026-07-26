# Registry schema

The registry is data. Adding a method is a file edit, never a code change. This document
is the contract every crate and every binding is written against.

Two populations live here and they are counted separately. **Computation entries** describe
something a machine calculates. **Protocol entries** describe something a human did to the
athlete or the recording. They have different shapes because a protocol has no rule to
evaluate, and forcing them into one shape is what makes method registries unusable.

## File layout

```
registry/
  constructs.toml          what is being measured, above the methods that measure it
  methods/<group>.toml     computation entries, one file per registry group
  protocols/<area>.toml    protocol entries, their own namespace and denominator
  instruments.toml         devices, their disclosed behaviour, and their biases
```

## A construct sits above a method

A construct is the quantity itself. A method is one published way of obtaining it. The
distinction is load-bearing rather than tidy: standing-frame and takeoff-frame jump height
carry a bias against each other of about 144 mm for the *same* method, because they are not
the same quantity. Filing them as two methods of one construct would state that they are.

```toml
[[construct]]
id = "jump_height.takeoff_frame"
title = "Jump height, centre of mass rise from the instant of takeoff"
unit = "meters"
frame = "takeoff"
notes = "Not comparable with standing_frame without a declared correction."
```

## A computation entry

`id` is the canonical dotted name and is the only stable identifier. Nothing else may be
used as a key.

```toml
[[method]]
id = "onset.threshold.noise_relative"
construct = "movement_onset"
group = "C"
title = "Noise-relative threshold onset"
rule = """
Onset is the first sample at which vertical ground reaction force departs the
quiet-standing mean by k standard deviations of that same epoch, then walked back
by a fixed offset.
"""
status = "accepted"        # recommended | accepted | contested | legacy | deprecated
confidence = "high"        # high | medium | low
debate = "genuine"         # genuine | vendor_or_legacy | single_position
```

### Parameters carry what the row grain deliberately does not

The row grain is per kind of rule, ratified 2026-07-25. One row is one idea. Every
paper-specific and tool-specific setting is a parameter on that row, never a row of its
own. This is the field that makes that ruling work.

`published_values` is the set the literature actually contains. It exists so the software
can answer "how many of the published settings can a user of tool X reach", which measured
across the seven open tools is one of six.

```toml
[[method.parameter]]
name = "k"
unit = "standard_deviations"
published_values = [2.0, 3.0, 5.0, 10.0]
default = 5.0
default_source = "owen2014"
required = true

[[method.parameter]]
name = "back_offset"
unit = "seconds"
published_values = [0.010, 0.030, 0.040, 0.050]
default = 0.030
required = true
notes = """
The offset changed with every publication from the originating lab, 10 ms in 2009 to
30 ms in 2014 to 50 and 40 ms in 2019, cause stated as currently unclear. An entry
citing only the k value does not identify a method.
"""
```

### Citations, and what each source actually did

`role` separates who proposed a rule from who merely used it, because a tool implementing
a published method is an implementation and not a variant.

```toml
[[method.citation]]
key = "owen2014"
role = "proposes"          # proposes | uses | evaluates | disputes
reference = "Owen et al. 2014, JSCR 28:1552-1558"
doi = "10.1519/JSC.0000000000000311"
obtained = false           # false means the claim rests on a secondary source
```

### Bias, and the criterion it was measured against

A bias figure is meaningless without the thing it was measured against. `criterion` is
mandatory. Two device-validation papers compute the reference plate's jump height *from
flight time*, so their biases are additive to flight-time method bias rather than inclusive
of it, and without this field a user silently double counts.

```toml
[[method.bias]]
magnitude = -0.026
unit = "seconds"
direction = "late"
criterion = "manual_identification.tillin2010"
criterion_kind = "human_visual"     # human_visual | instrument | simultaneous_capture | model
source = "guppy2021"
conditional_on_success = true       # see failure below
```

### Failure rate, because for some rules the disagreement is not a bias

Measured on the 244-trial corpus: two published onset rules place onset more than two
seconds before takeoff on roughly one trial in seven, on a movement lasting under a second.
Their medians are unremarkable and their 95th percentiles are three times normal. A single
bias figure for such a rule averages working with not working, so bias is reported
conditional on success and the failure rate is reported next to it.

`detectability` is a product-risk field, not a statistics field. A rule that returns an
absurd value fails loudly if anything is checking and invisibly if nothing is.

```toml
[method.failure]
rate = 0.149
numerator = 36
denominator = 241
corpus = "harrison2011_cmj"
definition = "time to takeoff exceeding 2.0 s on a movement whose true duration is under 1 s"
detectability = "silent"    # silent | loud | guarded
```

### Relationships

```toml
[[method.disagrees_with]]
id = "onset.threshold.percent_bodyweight"
kind = "genuine"           # genuine | vendor_convention | units | naming
note = "Different objective functions, and the field has not noticed."
```

### How the interface must treat it

Taken from the registry's GUI surfacing rulings. `refuse` exists because some combinations
are not a user choice and must not be offered.

```toml
[method.gui]
surfacing = "force_a_decision"   # default_and_hide | default_and_show | surface_on_demand | force_a_decision | never_a_user_choice | refuse
sensitivity = "high"             # how much the output moves when this choice moves
rationale = "Four published evaluations optimise four different objective functions."
```

## The fingerprint, and its acquisition block

The fingerprint is what proves two labs computed the same quantity. Ratified 2026-07-25: it
carries acquisition as well as analysis. Two results match only when both match.

A dataset that cannot fill the acquisition block fingerprints as **incomplete**, never as
matching. That is the intended behaviour and the reason the ruling was made: the most
consequential parameter in one open tool is a 50 ms contact debounce living in firmware,
silently mutable, and no reanalysis can recover from it.

```toml
[fingerprint]
analysis = ["method_id", "bound_parameters", "registry_version"]
acquisition = [
    "sample_rate_hz",
    "filter_at_capture",
    "tare_state",
    "plate_natural_frequency_hz",
    "floor_surface",
    "firmware_version",
]
incomplete_acquisition_is_never_a_match = true
```

## A protocol entry

Separate namespace, separate denominator, and no `rule` field. These are among the largest
levers in the field and none of them is a computation. One measured example from the
registry: a sentence spoken to the athlete moves peak rate of force development by 20 to
46 percent.

```toml
[[protocol]]
id = "protocol.arms.on_hips"
area = "execution"
title = "Arms held on the hips throughout"
description = "Athlete keeps both hands on the iliac crest from the weighing epoch to landing."
affects = ["jump_height.takeoff_frame", "movement_onset"]
provenance = "published"    # published | observed_from_code | vendor_documented
```

`observed_from_code` exists because the crosswalk finds protocol requirements that no paper
states. The clearest measured case: two tools assume the recording was trimmed to a single
jump, and on an untrimmed recording they place takeoff an average of 843 ms late, after the
athlete has landed, on 155 of 244 trials, with no warning. That is a real requirement that
was never written down, and it belongs in this namespace flagged as observed rather than
published.

## Rules the loader enforces

Validation is not advisory. A registry that fails these does not load.

1. Every `id` is unique across the whole registry, and dotted.
2. Every `construct` referenced by a method exists in `constructs.toml`.
3. Every `disagrees_with.id` resolves, and disagreement is symmetric.
4. Every `bias` has a `criterion`. No exceptions, including when the criterion is our own.
5. Every parameter with a `default` names the `default_source` that chose it.
6. Every `citation` with `obtained = false` bars the entry from `status = "recommended"`.
7. A `method.failure` with a `rate` must carry both `numerator` and `denominator`.
8. Counts are reported per population. Computation and protocol totals are never summed
   into a single number.
