# Registry schema

The registry is data. Adding a method is a file edit, never a code change. This document
is the contract every crate and every binding is written against.

Three kinds of entry live here and each is counted on its own denominator. **Computation
entries** describe something a machine calculates. **Protocol entries** describe something a
human did to the athlete or the recording. **Presets** name a published pipeline as a set of
bindings into the other two. They have different shapes because a protocol has no rule to
evaluate and a preset has no rule of its own, and forcing them into one shape is what makes
method registries unusable.

## File layout

```
registry/
  VERSION                  the revision the registry names itself, one line, optional
  constructs.toml          what is being measured, above the methods that measure it
  methods/<group>.toml     computation entries, one file per registry group
  protocols/<area>.toml    protocol entries, their own namespace and denominator
  presets/<source>.toml    published pipelines, their own namespace and denominator
```

Each directory is walked to the bottom, so a group may grow subdirectories without a code
change. A `.toml` anywhere else under the root belongs to no population and is refused by
name, because a method file dropped in the wrong place would otherwise contain entries that
silently do not exist.

`VERSION` is the revision a reader is meant to cite. It carries no entry and moves no count:
the walk filters on the `toml` extension, so this file is invisible to it. A blank or
unreadable file reads as no revision rather than as an empty one.

## A construct sits above a method

A construct is the quantity itself. A method is one published way of obtaining it. The
distinction is load-bearing rather than tidy: standing-frame and takeoff-frame jump height
carry a bias against each other of about 144 mm for the *same* method, because they are not
the same quantity. Filing them as two methods of one construct would state that they are.

```toml
[[construct]]
id = "jump_height.takeoff_frame"
title = "Jump height, centre of mass rise from the instant of takeoff"
label = "Jump height"
unit = "meters"
frame = "takeoff"
rules_answer = "one_question"    # one_question | their_own_questions
notes = "Not comparable with standing_frame without a declared correction."
```

`label` is the field's spoken words for the quantity, for surfaces that show a name beside
the identifier. Measured across six course documents, `takeoff` appears in 6 of 6 and
`onset`, `threshold` and `epoch` in 0 of 6, so the identifier alone reaches a reader who has
met the concept under other words.

### What a construct's rules are to each other

A request carries one rule per construct, so a construct holding several entries is a choice
the caller makes. Two shapes are sound and they are not the same choice.

- `one_question`, the entries report the same quantities and picking one moves the values.
  The five onset rules place one instant five ways.
- `their_own_questions`, each entry reports quantities the others do not, so picking one
  settles which quantities exist. The three phase models publish different landmark sets.

Mixing them is the fault. Some entries share a key, so they answer the construct's question
together, while another reports only keys those never report, so it answers a different
question from the same slot: a caller reaching for that one loses the others' quantities and
nothing on the result says so. `propulsion_subdivision` was cut out of `phase_model` for
being that shape.

`rules_answer` sits on the construct rather than on the entries because no single entry's
row can carry it. From one row nothing says what the row beside it answers, so putting it on
entries would store one fact N times and want a validator to keep the copies agreeing. It is
not a relation either: it does not vary pair by pair, and a relation would want an edge for
every pair and new edges on every pair a new entry creates. `disagrees_with` cannot serve for
a stronger reason than shape. Its four kinds, `genuine`, `vendor_convention`, `units` and
`naming`, all describe two entries answering the *same* question and returning different
numbers, which is the opposite claim.

Optional, and stated wherever the software can run two of a construct's entries and read the
answer back off a recording. The guard
`a_construct_holds_rules_that_all_answer_its_question_or_all_answer_their_own` requires it
there and refuses a construct whose measured entries contradict it, either way round. Adding
a second runnable entry to a construct that declares nothing turns that guard red, and its
message says what to write. Left off elsewhere on purpose: a declaration no measurement can
reach is an assertion, and this registry states queries.

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

A parameter varies by number or by name, never both. The number case carries
`published_values` with `default`; the name case carries `value` blocks with `default_key`.
Either default names the `default_source` that chose it, and a parameter declaring both is
refused.

`published_values` is the set the literature contains. It exists so the software can answer
"how many of the published settings can a user of tool X reach", which measured across the
seven open tools is one of six.

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
default_source = "owen2014"
required = true
notes = """
The offset changed with every publication from the originating lab, 10 ms in 2009 to
30 ms in 2014 to 50 and 40 ms in 2019, cause stated as currently unclear. An entry
citing only the k value does not identify a method.
"""
```

#### A parameter the literature varies by name

Some parameters have no numeric axis at all. A search signal is `velocity_argmin` or
`force_bw_crossing`; a regression is calibrated on one of ten stated populations. Neither
fits a list of floats, and both are choices that move the number.

```toml
[[method.parameter]]
name = "search_signal"
unit = "enumeration"
default_key = "velocity_argmin"
default_source = "mcmahon2018"

[[method.parameter.value]]
key = "velocity_argmin"
label = "Minimum centre of mass velocity"

[[method.parameter.value]]
key = "force_bw_crossing"
label = "Force returning through system weight"
notes = "Multi-valued on a real trace, and the disagreement tracks the re-crossing count."
```

One option may carry several numbers, which is what a set of published regression
coefficients is. Each number states its own name and its own unit, because a single set
mixes them: watts per centimetre beside watts per kilogram beside watts. One published
height term is per metre where the other nine are per centimetre, so a shape carrying bare
numbers would reintroduce a factor of a hundred.

```toml
[[method.parameter.value]]
key = "sayers1999_countermovement"
label = "Sayers et al. 1999, countermovement"
source = "sayers1999"

[[method.parameter.value.number]]
name = "jump_height_coefficient"
value = 51.9
unit = "watts_per_centimeter"

[[method.parameter.value.number]]
name = "body_mass_coefficient"
value = 48.9
unit = "watts_per_kilogram"

[[method.parameter.value.number]]
name = "intercept"
value = -2007.0
unit = "watts"
```

`unit` on a number is not optional. Dimensionless is a unit and saying so is what stops the
next author omitting it for a real one.

#### What `required` claims

`required` is a claim about the rule, not about the caller: the rule cannot produce a result
without a value for this parameter. It does not mean the request has to carry one.

Which gives two shapes, and the difference is whether a default sits beside it.

**Required with a default.** The registry supplies the value where the request names none,
and the record calls it `assumed` rather than the reader's own choice. The pair is what lets
the software say that a value the literature publishes six ways was chosen by nobody:
`onset.threshold.noise_relative` takes `k` as required at a default of 5, and a run that
never mentions `k` is answering a question it was not asked. This is the common shape and the
worked examples above both use it.

**Required with no default.** The request is refused by name, which is the honest shape for a
choice the literature does not settle, and setting a `default_key` on one would invent a
default to fill a field.

A parameter that is not required is one the rule runs without. Absent, no value binds for it
and nothing is assumed on the reader's behalf.

### Citations, and what each source did

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

A preset carries the same table under `[[preset.citation]]`. A protocol carries it under
`[[protocol.citations]]`, plural, and the singular spelling on a protocol does not load.

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

Some biases are not constants. A rule that waits a dwell before declaring stabilisation
overstates by exactly that dwell, so its bias is whatever the reader set the dwell to.
Measured across dwells of 120, 600, 1200 and 2400 samples, the overstatement was 120, 600,
1200 and 2400 samples. `equals_parameter` names the parameter of the same entry whose value
the bias equals, and `magnitude` then means the size at that parameter's declared default.

```toml
[[method.bias]]
magnitude = 1.0
unit = "seconds"
direction = "long"
equals_parameter = "dwell_seconds"
criterion = "time_to_stabilisation"
criterion_kind = "model"
```

The two cannot drift: the named parameter must exist on the entry, must declare a default,
must agree with the magnitude, and must be in the same unit.

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

Disagreement is a relationship rather than an annotation, so both sides record it. A reader
arriving from the silent side of a one-sided edge never learns there is an argument.

### What stands between an entry and a recording

Some entries cannot be computed on a given recording whatever the software does. That is a
fact about the entry, so it sits beside the entry rather than in a table somebody keeps in
step by hand.

```toml
[method.reach]
boundary = "equipment"     # protocol | equipment | both | source | undetermined
```

- `protocol`, a different movement on the plate the operator already owns.
- `equipment`, an instrument the lab does not have. The movement is not the barrier.
- `both`, a different movement and an instrument.
- `source`, no acquisition unblocks it: the rule text, constant or equation is not
  obtainable.
- `undetermined`, nobody has classified it.

An undetermined boundary carries the query that would settle it. A settled one does not,
because a question beside a classification that was made reads as doubt about it.

```toml
[method.reach]
boundary = "undetermined"
query = "the plate model and channel order from the 2011 thesis methods, or an AMTI config file"
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
analysis = ["method_id", "bound_parameters", "registry_digest", "registry_version"]
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

The two registry members answer different questions. `registry_digest` is taken over the
files that were read, so it names them whether or not anybody declared a revision, and two
registries differing by one edited rule differ in it. `registry_version` is the revision a
caller pinned, and it is absent when nobody pinned one.

A result carries a third registry field, `registry_declared_version`, and it is not a
fingerprint member. It is the revision the registry names about itself, from the `VERSION`
file beside its rules. Three rules govern it:

- **It is never written into `registry_version`.** A reader takes a value there as the author
  having chosen a revision, so publishing the registry's own claim under that name tells every
  reader who checks the provenance something that is not true, and tells the reader who
  ignores it nothing wrong at all. The terminal and the browser both did this until 2026-08-03.
- **It is not recoverable from the digest**, so it is carried rather than derived. The walk
  that measures the digest reads only the `toml` files, and the revision lives beside them, so
  two registries with byte-identical rules and different revisions share one digest.
- **It is outside the fingerprint on purpose.** Two labs whose rule bytes are identical
  computed the same quantity whatever their `VERSION` files say, and the digest above already
  separates labs whose bytes differ. Hashing the claim would break every match already
  recorded against those rules the first time somebody edited a `VERSION` file alone.

Unpinned, `registry_version` is written as null rather than left out, and every surface
carries the key on every result. A key a document sometimes omits cannot be told apart from a
field a surface never carried, and nothing in the document says which happened. The same rule
governs the batch record, whose `registry_version` was typed as a string and wrote `""` for an
absent pin until 2026-08-04, so a run nobody pinned and a run pinned to the empty string read
alike.

### Which surface asks for the block

A trace of forces carries none of the acquisition block, so it is stated by whoever runs the
analysis and no surface can derive it. The terminal takes `--acquisition <member>=<value>`,
repeatable, on a single trial and on a folder run; the R and Python trials take an acquisition
argument; the browser module takes the block beside the request.

### Saving a plate rather than retyping it

A lab has one or two plates whose firmware changes rarely, so a bar that asks five questions at
every analysis asks the same answers hundreds of times. `plateforce plate save <name>` records
them once and `--plate <name>` fills the block from them.

```
plateforce plate save lab-kistler-1 --acquisition filter_at_capture=none ...
plateforce analyse trial.txt --plate lab-kistler-1
```

Three rules keep a saved plate from becoming a second home for the fact.

- **A record carries the members, never a reference to them.** A result that travelled away
  from the machine that saved the plate holds every member it ran under.
- **A member stated beside a plate is the answer that runs**, because stating one is a caller
  producing evidence and reading a saved plate is not. What it displaced reaches the record
  under `plate_profile.superseded_members`, so a reader sees both numbers.
- **Neither the name nor the revision is fingerprint material.** What a lab calls its own plate
  is not a fact about the capture, and two labs whose plates are configured alike have to match
  whatever they file them under. `run_fingerprint` clears `plate_profile` before it digests the
  run row for exactly this.

`plate_profile.revision` is the digest of the members the plate held when it was read, so two
results taken off one plate name either side of an edit differ visibly rather than silently.
`plateforce plate show <name>` prints the revision a plate hashes to now, which is what a reader
holding an older result compares against. Saved plates live where the operating system keeps a
program's settings, per user, and `--plates <DIR>` names a folder instead, which is how a plate
travels with a dataset rather than with the person.

### What a run publishes when the block is unfilled

Nothing. `run_fingerprint` is null on a run whose acquisition block is short of a member, and
carries the digest when the block is filled. A marked digest was the alternative and it fails
the rule above: two runs off two differently configured plates render one string and compare
equal, which is the matching the ruling forbids. `acquisition_complete` says which case a
reader is looking at, and `acquisition.missing` names what would fill it.

What this costs is that a reader cannot see that two incomplete runs performed the same
computation. That is the inference the ruling denies, because the settings deciding whether
two labs agree were never recorded.

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

[[protocol.citations]]
key = "kirby2011"
role = "proposes"
reference = "Kirby et al. 2011, JSCR 25(9):2510-2518"
obtained = true
```

`observed_from_code` exists because the crosswalk finds protocol requirements that no paper
states. The clearest measured case: two tools assume the recording was trimmed to a single
jump, and on an untrimmed recording they place takeoff an average of 843 ms late, after the
athlete has landed, on 155 of 244 trials, with no warning. That is a real requirement that
was never written down, and it belongs in this namespace flagged as observed rather than
published.

## A preset

A published pipeline under a name, so `--preset owen2014` is the pipeline that paper ran.
Separate namespace, separate denominator, no `rule` field and no `surfacing` field, for the
same reason a protocol has neither: it describes which rules to use, not how any of them
works.

**A preset binds only the slots its source states.** A slot the source is silent about is
left to the software's normal resolution and is never attributed to the preset. A preset
that filled in the rest would manufacture provenance, which is the defect this project
records in a competitor: a settings screen naming a method and a citation over a code path
computing something else.

```toml
[[preset]]
id = "owen2014"
title = "Owen et al. 2014"
description = """
The criterion specification Owen et al. paired: system weight over a one-second window,
movement onset five standard deviations of that window's noise, and the crossing stepped
back thirty milliseconds.
"""
states_nothing_about = ["takeoff"]

[[preset.binding]]
construct = "system_weight"
method_id = "bwepoch.fixed_window"
parameters = { duration = 1.0 }

[[preset.binding]]
construct = "movement_onset"
method_id = "onset.threshold.last_within_band"
composed_from = "onset.threshold.noise_relative"
parameters = { k = 5.0 }
note = "The instant taken is the last sample still inside the band rather than the first outside it, which is the crossing operator bound to last."

[[preset.citation]]
key = "owen2014"
role = "proposes"
reference = "Owen et al. 2014, JSCR 28(6):1552-1558, the criterion pairing and 30 ms"
doi = "10.1519/JSC.0000000000000311"
obtained = false
```

A binding states the numbers its source states in `parameters` and the named options in
`options`, keyed by parameter name. `states_nothing_about` names constructs the source is
silent about, as a fact about the source rather than about this software. Each must be a declared construct: a misspelling
there would read as a source saying nothing about a quantity, with the software agreeing.

`composed_from` names the entry a composed `method_id` composes. A composition is an entry
with an operator bound onto it, and it carries that entry's citations rather than holding a
row of its own, so it has nothing for a validator to resolve against. Without this field an
id that composes and an id that is a typo are the same thing to the loader.

A preset naming a method the registry does not carry does not load. A preset naming a method
that exists but has no rule behind it does load, because the alternative would make the
registry unloadable the moment a preset cites an entry whose rule has not landed, freezing
breadth behind preset maintenance. That case is a refusal where the user asked for it,
carrying the same names a load failure would.

## Rules the loader enforces

Validation is not advisory. A registry that fails these does not load. Each names the
`ViolationKind` a caller matching on the violation will see.

**Identity and reference**

1. Every `id` is a dotted canonical name. `IdNotDotted`
2. No `id` appears in more than one population, and no population defines one twice.
   `DuplicateId`
3. Every `construct` a method names exists in `constructs.toml`, and everything a protocol
   `affects` is a construct or an entry the registry carries. `UnknownConstruct`

**Disagreement**

4. Every `disagrees_with.id` resolves. `UnknownDisagreement`
5. Disagreement is symmetric. The named entry disagrees back.
   `AsymmetricDisagreement`

**Parameters and their defaults**

6. A parameter with a default names the `default_source` that chose it, whether the default
   is a number or a name. `DefaultWithoutSource`
7. A `default_source` names a citation this entry carries. A key resolvable only on some
   other entry, or nowhere, leaves a reader holding a value and a word rather than a source.
   `DefaultSourceNamesNoCitation`
8. A parameter declares one default, not two. `DefaultDeclaredTwice`
9. A `default_key` names one of the options that parameter lists.
   `DefaultNamesUnknownValue`
10. No parameter lists one option key twice. `NamedValueDeclaredTwice`

**Bias**

11. Every bias has a `criterion`. No exceptions, including when the criterion is our own.
    `BiasWithoutCriterion`
12. A bias that equals a parameter names one this entry carries.
    `BiasNamesUnknownParameter`
13. That parameter declares a default, or the magnitude is anchored to nothing.
    `BiasNamesParameterWithoutDefault`
14. The stated magnitude equals that parameter's default.
    `BiasMagnitudeDisagreesWithParameter`
15. The bias and the parameter are in the same unit. An identity between quantities in
    different units is not an identity. `BiasUnitDiffersFromParameter`
16. A `criterion` names an entry, a construct, the entry itself, or one of the external
    criteria the vocabulary declares: `motion_capture_marker`, `rubber_band_goniometer`,
    `static_dead_weight_calibration`. The list is closed because an open field reads a
    mistyped name as a fourth instrument and loads. `BiasCriterionUnresolved`

An entry naming itself is three claims sharing one spelling, and `criterion_kind` says which.
`model` compares two settings of this entry's own parameter. `instrument` is two
implementations of this one rule disagreeing, which needs no parameter. `human_visual` is the
definition of record, whose figure is the reference's own spread rather than a bias.
`simultaneous_capture` is the entry's own design against departing from it. Each carries the
claim its kind makes:

17. A `model` self-comparison declares a parameter to have two settings of.
    `SelfComparisonSweepsNoParameter`
18. A `human_visual` self-comparison reports no direction but `none` or `either`, because a
    reference is not biased against itself. `DefinitionOfRecordCarriesADirection`

**Failure rate**

19. A `method.failure` carries both `numerator` and `denominator`.
    `FailureWithoutDenominator`
20. The stated `rate` matches its own numerator over its own denominator, within a tolerance
    loose enough for a rounded literal and tight enough to catch a transcription error.
    `FailureRateInconsistent`

**Status and interface**

21. A `citation` with `obtained = false` in a load-bearing role bars the entry from
    `status = "recommended"`. `RecommendedOnUnobtainedSource`
22. `surfacing = "refuse"` carries a `gui.rationale`. Every other verdict decides its own
    behaviour; refusing decides only that the rule is not offered, so what a reader is owed
    instead has nowhere else to live. `RefuseWithoutRationale`

**Reach**

23. A query sits beside an undetermined boundary and nowhere else.
    `ReachQueryOnSettledBoundary`
24. An undetermined boundary carries a query. Every other boundary names what stands in the
    way; this one names only that something does, so the query is the whole of what it says.
    `ReachUndeterminedWithoutQuery`

**Presets**

25. A preset binds a method the registry carries. `PresetBindsUnknownMethod`
26. A binding's declared construct is the one that entry carries.
    `PresetBindingConstructMismatch`
27. A preset states a pipeline and cites a source for it. `PresetWithoutCitation`
28. A preset binds each construct once. `PresetBindsOneConstructTwice`
29. Everything a preset says its source is silent about is a declared construct.
    `PresetSilentAboutUnknownConstruct`

A closed vocabulary needs no rule. `status`, `confidence`, `debate`, `criterion_kind`,
`detectability`, `provenance`, `surfacing`, the disagreement kinds and `reach.boundary` are
enums, so a spelling outside the set does not parse. `deny_unknown_fields` is set throughout
for the same reason: a misspelt key is refused rather than dropped, because a dropped key is
a setting a reader believes they stated.

## What the loader refuses before it validates

A registry has to exist and describe itself before its entries can be judged. These refusals
are the same code on every surface, so the browser cannot assemble a file set the desktop
would reject.

1. A root that is absent, or is not a directory. An empty load reports zeros and no
   violations, which reads as a registry that passed.
2. A root holding no methods. A sparse checkout or a `methods/` directory that failed to sync
   reaches that state.
3. Two entries carrying one id. Inserting into a map means the second replaces the first, so
   a census of surviving keys stops being a census of what the files declare.
4. A `.toml` no population owns.
5. A symlink under the root that leads back to a directory already being walked.

## How counts are reported

Every count carries the population it counts over. Computation entries, protocol entries and
presets are counted apart and are never summed into one total, and a figure taken over a
subset states its denominator: `122 of 259 genuine debates`, never `122`.

```bash
cargo run -q -p plateforce-cli -- registry census
cargo run -q -p plateforce-cli -- registry validate
```

## One wire value never means two things

A result carries the method that produced it, so a reader who checks the provenance has to
end up better informed than one who does not. A document where two states share a spelling
does the opposite: it misleads exactly the reader who looks. The rule below is the general
form of a thing this repository had already settled twice in particular cases, written down
because it had not been.

**Where two states could share a spelling, the document carries a field that says which.**
The field is always written, never omitted in one of the states, for the same reason
`registry_version` is written as null rather than left out: a key a document sometimes omits
cannot be told apart from a surface that never carried it.

Three instances, and all three are `null` meaning two things.

- **A number that is not a number, against a number nobody computed.** `serde_json` writes a
  non-finite float as `null`, exactly as it writes an absent one, so "the software computed
  this and got not a number" and "the software declined to compute this" arrived at every
  reader identically. The first is a gap in the recording reaching a quantity; the second is
  an honest refusal that `refusals` accounts for by name. Every metric therefore carries
  `carried_no_number`, and `value` is a finite number or nothing at all. Measured on
  `subject01_trial1_interrupted`: 8 of 11 metrics read `null` and 2 of those 8 are the first
  state, being the two quantities computed over the weighing window that holds the
  recording's three unreadable samples.

- **A level that has no number, against a level nobody drew.** The same `null`, and until it
  was fixed the two members of `levels` most likely to hold a non-finite number were declared
  as plain numbers, so the pair whose type promised most were the pair that lied. Every
  member of `levels` is now optional and none of them is ever a value that is not finite.

- **A sample the reader was told to treat as missing, against a sample the recording lost.**
  Not `null` but one integer, and worse for it, because the number looked like an answer. The
  zero convention a vendor writes for a measurement it does not have is also the correct
  reading of an unloaded plate, so on a jump trace it matches the whole flight phase. One
  total read **160** on a recording whose real answer is 157 samples of an athlete in the air
  and 3 samples of a gap. The two are counted apart by
  `plateforce_core::signal::reported_samples`, which is the one home for that policy on every
  surface: the reader publishes what its declared convention matched, and the engine publishes
  `samples_carrying_no_number` over the recording, so a notebook and an R session carry the
  count as well as a terminal and a browser tab.

**Counts obey the rule above them.** Two reasons a sample is reported are two populations, and
adding them produces a figure with no denominator anybody can name.
