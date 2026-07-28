# Code conventions

Binding on every file in the software repo. Written before any code exists so
nothing has to be un-learned later.

This project is written by AI and published in public. Nothing in the source
should announce that, apologise for it, or read like a transcript of the session
that produced it. The code does the thing. That is all it does.

---

## 1. Comments

**Density.** One line every 10 to 30 lines, where the intent is not already
obvious from the code and the names. Long stretches of self-evident code get
nothing. A file with a comment on every third line has failed.

**Length.** One line. Almost always. If a comment needs three lines, the code
underneath it probably needs a better name or a split.

**Content.** State what the code does in the wider system, or the fact a reader
cannot recover from the code. Not the mechanism the next line already shows.

```rust
// GOOD  Threshold families are stored per axis so `disagrees_with` cannot pair across axes.
// GOOD  Flight-window residual is nonzero on a real plate; see registry entry takeoff.threshold.
// BAD   Loop over the samples and find the first crossing.
// BAD   This handles the case where the baseline is noisy.
```

**Banned outright.**

| banned | why |
|---|---|
| development diary | "Fixed the race where...", "Refactored to handle...", "IMPROVED (Jan 2026)". The reader was not there and does not care. |
| conversational voice | "We need to do this because...", "This handles the case where...", "Note that we..." |
| ALL-CAPS emphasis | IMPORTANT, CRITICAL, KEY INSIGHT, NOTE. Reserved for genuine data-loss or safety boundaries, and nowhere else. |
| restating the next line | The code speaks. |
| defensive justification | Design decisions get one line or a docs page, never a paragraph in the source. |
| bindingness or doctrine | No DECREE, LEAN, AI-LEAD, OPEN, "ratified", "canonical", "by design". Those live in `docs/`, never in source. |
| hardening language | verified, confirmed, proven, definitely, certainly, intentional, guaranteed, ensures, correct, safe to. If it were proven the code would show it. |
| em dashes | Commas, colons, periods. |

**The one exception to brevity.** A comment that records a *provenance* fact
earns its line and sometimes two, because it cannot be recovered from the code:

```rust
// Constant is Sams' disclosed value; the author flags it as arbitrary and
// mass-absolute where it should be mass-relative.
```

That is not narrative. That is the fact that makes the constant auditable.

---

## 2. Names

**Descriptive, verbose, disambiguated. Prefer the field's standard name in every
case where one exists.** A longer name that cannot be confused with a sibling
beats a short one that can.

The registry is full of quantities that differ by a single qualifier and are
routinely conflated in the literature. The code must never reproduce that
conflation, and the names are the main defence.

| write | not | because |
|---|---|---|
| `vertical_ground_reaction_force_newtons` | `force`, `f`, `grf` | there are several forces; only one is vGRF |
| `system_weight_newtons` | `bodyweight`, `bw`, `weight` | system weight includes the bar; bodyweight does not |
| `takeoff_velocity_meters_per_second` | `velocity`, `v` | takeoff velocity and peak velocity are different instants |
| `jump_height_from_takeoff_meters` | `jump_height` | the standing frame differs by 26 to 45 percent |
| `onset_index` / `onset_time_seconds` | `start` | say which of the two, and say the unit |
| `weighing_epoch_duration_seconds` | `window` | there are at least four windows in the pipeline |
| `braking_phase_start_index` | `phase_start` | six phase boundaries exist |
| `net_peak_force_newtons` | `peak_force` | gross and net differ by exactly one bodyweight |
| `sample_rate_hz` | `rate`, `fs` | `fs` is standard in DSP but ambiguous next to force |

**Units in the name** for every physical quantity, unless the type already
carries them. `_newtons`, `_seconds`, `_meters`, `_hz`, `_kilograms`,
`_meters_per_second`, `_watts`. This is verbose on purpose. Unit confusion is a
documented defect class in this literature and the registry records real
instances of it.

**Standard names win.** Where the field has settled on a term, use it exactly:
`impulse`, `rate_of_force_development`, `reactive_strength_index_modified`,
`countermovement_jump`, `isometric_midthigh_pull`. Do not invent a synonym and
do not abbreviate a term the field spells out. Where the field is split, the
identifier carries the qualifier: `eccentric_phase` never appears without the
convention it belongs to.

**Registry identifiers** stay in their canonical dotted form as data, never
transliterated into code identifiers: `onset.threshold.noise_relative` is a
string key, not `OnsetThresholdNoiseRelative`.

---

## 3. Structure

**The math has one home.** A quantity is computed in exactly one place. If two
call sites need it, they call the same function. The entire premise of this
project is that independent implementations of the same named method disagree;
shipping two of our own would be indefensible.

**Method definitions are data, not code.** Rules, citations, status flags, bias
magnitudes and parameters live in the declarative registry. Adding a method must
not require a compiler. If a method's rule text is embedded in a function body,
it is in the wrong place.

**Errors say which method and which parameter.** A failure that says
`threshold not crossed` is useless. `onset.threshold.noise_relative(k=5) found
no crossing within the search bound` is actionable.

**No silent exclusion.** Anything that drops a trial, a candidate or a sample
reports what it dropped and under which rule. Silent data exclusion is the
failure mode the registry exists to prevent, and it must not appear in our own
code.

**No silent inclusion either. A sentinel is not a measurement.** Vendor exports
encode missing data as `0`, `-1`, `9999` or an empty cell, and reading one as a
real value is the same defect wearing the opposite costume. Every reader declares
which sentinel convention it applied and reports what it treated as missing.

```
// Three sentinel rows in 244 moved a published correlation by 0.16;
// the commercial export writes 0.00 to height, time and RSI together.
```

**A method that can fail reports its failure rate, not just its bias.** Two of the
nine onset rules in this corpus place the movement start more than two seconds
before takeoff on roughly one trial in seven. A single bias figure for such a rule
averages working and not working, so bias is reported conditional on not having
failed, alongside the failure rate and its denominator.

---

## 4. Repository files

`README.md` states what the software does and how to run it. Not how it was
built, not why it was built, not what was learned.

`NOTICE` carries copyright, licence posture and the dependency licence table.

`docs/` carries the reasoning, the registry documentation, the method rulings
and anything with a bindingness grade. All narrative goes here and none of it
goes in source.

`CHANGELOG.md` records what changed for a user. Not what a session did.

Commit messages state the change and its user-visible effect. No session
narrative, no discovery story.

---

## 5. Interface copy

Every string a user reads, in the browser, the command line, an error, or a
Python exception, describes their data and their choices. It never describes the
state of this software.

Banned outright: "in this build", "not implemented yet", "coming soon", "build
default", "N of M rules available here", "listed disabled rather than hidden",
"generated in this tab", and any panel named after the build.

Absence is not content either. The interface does not enumerate what it cannot
do, does not offer, or has withheld. A method with a rule behind it is offered;
one without is absent, and the interface says nothing about it. Coverage limits
are stated once, in `README.md`, where somebody deciding whether to adopt this
will look for them.

The distinction that makes this workable: a failure rate, a bias, a missing
citation and a contested status are facts about the *literature*, and they are
the product. "We have not written that part yet" is a fact about *us*, and it
belongs in the README. An error message may say what went wrong and what is
available instead. It may not apologise for the roadmap.

---

## 6. Enforcement

Run `/uncomment` over any file before it is committed for the first time, and
`/comment-truth` over any file whose comments were written before this document
existed. A pull request that adds net comment characters without adding
proportionate code is reviewed for bloat first and correctness second.
