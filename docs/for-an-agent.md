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

One call. It returns the schema name and version, the operations this build dispatches, every
rule it can run with the slot and construct each fills, the operator entries those rules compose
and the names you state to reach them, the acquisition block's members, the container formats
this surface writes, and every refusal code with its exit code.

**What it does not yet carry is the values you may state on each rule.** For those, ask the
registry per entry:

```
plateforce registry show <method_id> --format json
```

That returns the parameters with their units, their published values, their defaults and whether
the rule refuses without them. Learning the full picture therefore costs one call plus one per
rule. That cost is measured and held to a ceiling by
`crates/plateforce-cli/tests/discovery-calls.txt`; when the manifest carries parameters the number
becomes 1 and this paragraph changes with it.

**Read the registry rather than this page for anything about a specific rule.** Adding a method
here is a data edit, so the set of rules changes without the software changing, and a list of
them written into a document is stale the first time somebody adds one.

## What you may ask for

<!-- checked-against-capability: operations -->
```
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
row per trial per rule. `spread` sweeps a quantity over every rule on its path, which is the
command that shows how far the method choice moves the number. `reach` reports which constructs
this build can compute and what stands in the way of the rest.

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

## What every refusal means

Refusals carry a code and an exit code. Branch on the code, not on the text.

<!-- checked-against-capability: refusal_codes -->
```
ambiguous_force_channels
collapsed_band
column_not_found
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

**An argument the parser does not recognise also exits 64**, which is the exit code
`decision_not_made` and `conventions_not_comparable` use. So the exit code alone does not tell a
missing operation from an open decision. Read the message: the tool's own refusals begin
`plateforce:` and argument errors begin `error:`.

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
