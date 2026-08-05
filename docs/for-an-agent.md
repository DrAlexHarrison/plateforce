# plateforce, for an agent

This is the contract, not an introduction. It is written for a program driving the terminal on
somebody's behalf, and every list in it is checked against the software's own manifest by
`scripts/check-agent-contract.py`, which fails if this page names an operation that does not
exist or omits one that does. **Where this page and `plateforce capability --format json`
disagree, the manifest is right and this page is a bug.**

## The one thing to understand first

Published methods for the same jump metric disagree enough that the choice of method moves the
number further than a training intervention does. So a number without the rule that produced it
is not an answer here, and this tool will not give you one. Every value you get back carries the
method id, the parameters, where each parameter came from, the registry revision and its content
digest. When you report a number to whoever asked you, carry that with it.

## Discover at runtime, not from this page

```
plateforce capability --format json
```

One call, and it is the whole picture. It returns the schema name and version, the operations this
build dispatches, every rule it can run with the slot and construct each fills, the operator
entries those rules compose and the names you state to reach them, the acquisition block's
members, the container formats this surface writes, and every refusal code with its exit code.

**And every value you may state, on each rule and on each operator entry.** A `parameters` array
sits on each, and each entry carries `states`, the exact token you write, so you never have to
build the string yourself:

```json
{"states": "onset.k", "name": "k", "unit": "standard_deviations",
 "published_values": [2.0, 2.5, 3.0, 4.0, 5.0, 8.0],
 "named_values": [], "default": 5.0, "default_key": null, "required": true}
```

`published_values` is what the literature states, so more than one is a choice you have to make
rather than a range you may pick from. `named_values` carries the keys where the options are
names. `required` with no default is the shape that refuses until you state it.

Write it with `--set <states>=<value>`, which for the record above is `--set onset.k=5.0`.

That cost is measured and held to a ceiling by `crates/plateforce-cli/tests/discovery-calls.txt`,
which reads 1 for 107 rules. It read 108 until the manifest carried this.

The registry remains the one home for everything else about a rule, its title, its prose, its
citations, its disagreements and the notes on each parameter, and per-entry lookup is still how
you read those:

```
plateforce registry show <method_id> --format json
```

`--registry <dir>` reaches `capability` as it reaches every other command, and the values reported
are the ones that registry publishes. Point them at different directories and they will tell you
different things, correctly.

**Read the registry rather than this page for anything about a specific rule.** Adding a method
here is a data edit, so the set of rules changes without the software changing, and a list of
them written into a document is stale the first time somebody adds one.

## What you may ask for

<!-- checked-against-capability: operations -->
```
aggregate
analyse
batch
capability
compare
parse_force_file
reach
registry_census
registry_show
registry_validate
spread
version
```

`analyse` is one trace. `batch` is a folder, one row per trial, with the tables and the record
written to `--out-dir`. `compare` is the same folder under several rules for one quantity, one
row per trial per rule. `aggregate` reduces an athlete's trials to one number under a named
registry rule. `spread` sweeps a quantity over every rule on its path, which is the command that
shows how far the method choice moves the number. `reach` reports which constructs this build can
compute and what stands in the way of the rest.

**The list above is the terminal's, and the surfaces differ.** Each reports only what it can
actually do, so an operation missing from one surface's array is missing from that surface. Read
the array from the surface you are driving rather than from this page, and if you need an
operation it does not report, drive one that does.

## What every answer contains

A JSON answer is one object with a single key, `ok` or its refusal counterpart. Never parse the
text rendering; pass `--format json` and read that. `--out <path>` is the reliable route on
Windows, because PowerShell 5.1 writes UTF-16LE with a BOM through `>` and `json.load`, `jq`,
`pandas.read_json` and `jsonlite` all reject it.

Every quantity comes with a chain naming the rule that produced it, every parameter that rule
bound, and where each value came from: `stated` when the caller said so, `assumed` when the rule
or the registry supplied it, `measured` when it was read off the recording. **A value recorded
`assumed` is a choice somebody did not make**, and if the number matters, make it.

A folder run reports its own coverage with denominators: how many files were seen, how many
carried a declared trial suffix, how many were named, how many produced numbers, how many were
refused and how many a gate excluded. **Never report a count from this tool without the
denominator beside it**, because the denominator is the part that tells anyone whether the number
means anything.

`results.csv` and `results.parquet` are one table in two containers, with the same columns in the
same order: `trial_id`, `subject`, `source_path`, `provenance_id`, `refusal_code`, then one column
per quantity. **Group by `subject` rather than parsing `trial_id` yourself.** The run resolved the
athlete from the pattern you declared, and re-deriving it is this software's identity rule
reimplemented by you and free to disagree with it. It is empty when the run declared no pattern,
which means there is no grouping to do, not that the athlete is unnamed. **A trial that produced
no numbers keeps its subject**, so an athlete's denominator includes the trials that refused;
count those rows rather than dropping them, and `refusal_code` says why each one is there.

`provenance_id` is the join back to `provenance.csv`, which carries one row per method per
parameter with `source` reading `stated`, `assumed` or `measured`. That join is how a value in the
table keeps the rule that produced it.

**`descriptions.csv` is the long-form table, and it is the one to filter a cohort question over.**
One row per trial per quantity: `trial_id`, `subject`, `quantity`, `value`, `method_id`,
`provenance_id`, `account`. The number, whose trial it was, and the rule that produced it are all
on the row, so grouping, splitting and relating need no join. `account` is that number's own
account of itself in prose. The join through `provenance_id` is still how you reach every
parameter the rule bound.

Same columns in `descriptions.parquet`, with `value` a float rather than text.

**If you filter or concatenate these tables, carry `run.json` with them.** It holds the registry
digest, the request digest and the run fingerprint. A table separated from its record is a set of
numbers whose method nobody can recover, which is the failure this tool exists to prevent.

## What every refusal means

Refusals carry a code and an exit code. Branch on the code, not on the text.

<!-- checked-against-capability: refusal_codes -->
```
ambiguous_force_channels
collapsed_band
column_not_found
command_line_not_parsed
conventions_not_comparable
decision_not_made
dependency_unresolved
file_not_read
method_not_implemented
no_crossing
not_enough_observations
observations_not_paired
parameter_not_finite
plate_not_level
registry_invalid
required_parameter_unstated
schema_unsupported
sentinel_convention_unknown
trace_too_short
trial_identity_unparsed
unknown_parameter
value_not_accepted
```

**`decision_not_made` is the one to expect and it is not an error.** It means the rule you named
publishes several values for a parameter and the tool will not pick one for you. The refusal names
the parameter and lists the published values. Choose one, state it with `--set`, and say in your
answer which you chose.

**`command_line_not_parsed` is the one raised before any rule runs**: a word this build does not
offer, a required one you did not write, or two that cannot be written together. Its message is
the parser's own sentence and names the token you wrote, usually with the nearest thing that does
exist. It means re-read the manifest and rebuild the call, which is the opposite of what
`decision_not_made` means, and the two are worth keeping straight.

**Several codes share exit 64, so branch on the code and never on the status alone.**
`command_line_not_parsed`, `decision_not_made`, `required_parameter_unstated` and
`conventions_not_comparable` are all EX_USAGE and all mean something different. `file_not_read`
takes 66 and `registry_invalid` takes 78, because a workflow manager that retries on bad data and
stops on a missing file cannot tell those apart while they share a status.

Refusals are written to stderr, whichever channel raised them, because a refusal carries no
document and redirecting the document must not lose the reason one is absent. With
`--format json` you get the same `{"refusal": {...}}` envelope either way; without it you get
prose, because a person reading a terminal is not helped by an envelope.

## What this tool will not answer, and why silence is never the response

**It will not compute a quantity no published rule defines.** Ask for one and it refuses by name
and lists every construct it does run. It will not resolve your ask to the nearest thing it has.
If you want a number it does not carry, you cannot get it here, and the honest thing to report is
that the construct does not exist rather than a neighbouring number.

**It will not pick a contested value for you.** See `decision_not_made` above.

**It will not reduce trials to one number without being told which published rule to use.**
`trial.aggregation` publishes three and none of them is the arithmetic mean of a subject's trials,
so `--aggregate` takes a rule name and `--aggregate-n` takes the count the rule was asked for.

**It will not tell you a folder was fine when it was not.** A trial that could not be read or that
a rule declined is named in the refusals and stays in the denominator.

**And it never answers with nothing.** Every call either produces what was asked for or says what
it lacks in words naming the thing you asked for. If you ever get an empty answer and a zero exit
from a call that should have produced something, that is a defect in this software and worth
reporting, not a result to pass on.

## What is stable

The schema name and version at the top of the manifest tell you which shape you are reading. The
refusal codes and their exit codes are generated from the software's own vocabulary and are the
safest thing to branch on. Method ids come from the registry, which is data, so treat the set as
open and look up what you meet rather than pattern-matching on names.

No version of this software has been tagged yet, so nothing here has shipped to anyone and the
shapes above may still move. When a `v*` tag exists, this paragraph is the one to rewrite.

## The private path is the only path

Athlete attributes and any subject table stay on the machine they were read on. Do not copy
subject identifiers, attribute tables or per-subject values into anything you send anywhere.
