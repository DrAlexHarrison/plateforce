# plateforce at a terminal

The program teaches itself. `plateforce --help` is the whole map, every command carries examples
that run, and `plateforce man` writes the manual pages. This page is for the parts a help screen
is the wrong shape for: getting the manual and the completions onto a machine, and the two
questions a new reader always asks in the wrong order.

For a program driving the terminal on somebody's behalf, `docs/for-an-agent.md` is the contract
and this page is not.

## The first two minutes

```
plateforce --help
plateforce analyse jump.txt --column 0 --sample-rate-hz 1000 --sentinel none
```

The second refuses, and the refusal is the software working. It prints the choices still open on
the path to a jump height, every rule available for each, and the values the literature
published each rule at. Nothing is guessed, so nothing runs until somebody has chosen.

Two ways past it. Name a published pipeline, which answers every choice out of one source:

```
plateforce analyse jump.txt --column 0 --sample-rate-hz 1000 --sentinel none --preset sams
```

Or answer them one at a time, which is what the refusal was asking for:

```
plateforce analyse jump.txt --column 0 --sample-rate-hz 1000 --sentinel none \
  --weighing bwepoch.fixed_window --set weighing.duration=1.0 \
  --onset onset.threshold.noise_relative --set onset.k=5 \
  --takeoff takeoff.threshold.absolute_force --set takeoff.threshold_n=20
```

The line is long because none of it is assumed. Every one of those words appears in the result's
record, which is what lets somebody reading the number a year later know how it was made.

## Where the names come from

The method flags take registry ids, and the registry is data, so the set moves without the
program changing. One command prints the set a build carries:

```
plateforce methods              every step, and the rules under each
plateforce methods --slot onset the rules --onset takes, and nothing else
```

The heading over each block is the text to write. Three steps have a flag of their own,
`--weighing`, `--onset` and `--takeoff`. Everything else is reached by construct, through
`--derive braking_phase_start=<METHOD>` or `--condition conditioned_force_signal=<METHOD>`, and
the heading spells that out.

One rule in full, with its own words, its citations, every parameter and every value each
parameter takes:

```
plateforce registry show onset.threshold.noise_relative
```

## The manual pages

There is no package manager placing them, so the program writes them:

```
plateforce man
```

That writes one page per command, twenty of them, into `~/.local/share/man/man1`, or into
`$XDG_DATA_HOME/man/man1` where that is set. It then prints two commands. On Linux, where
`~/.local/share/man` is on the default manual path, `man plateforce` works straight away. On
macOS it is not, so use the second, which names the directory and works anywhere:

```
man -M ~/.local/share/man plateforce
man -M ~/.local/share/man plateforce-analyse
```

To put them somewhere the system already reads, name it:

```
sudo plateforce man --out-dir /usr/local/share/man
```

Windows has no `man`. The same content is `plateforce <command> --help`, which is where the
pages are generated from.

## Completions

```
plateforce completions bash
plateforce completions zsh
plateforce completions fish
plateforce completions powershell
plateforce completions elvish
```

Each writes the script to the terminal. For the current shell only, bash and zsh read it as it
arrives:

```
source <(plateforce completions bash)
```

Kept between sessions, it goes in a directory the shell reads at startup. `--out-dir` writes the
file under the name that shell looks for and prints the path:

```
plateforce completions bash --out-dir ~/.local/share/bash-completion/completions
plateforce completions fish --out-dir ~/.config/fish/completions
plateforce completions zsh  --out-dir ~/.local/share/zsh/site-functions
```

For zsh the directory has to be on `fpath` before `compinit` runs.

## Two widths of help

`-h` is the summary: what the command does in a line, its flags in a line each, and one or two
examples. `--help` is the whole thing: what the program is for, each flag's full note, and every
example with the prose that says why you would run it.

`plateforce help <command>` is the same as `plateforce <command> --help`.

## What a redirected result is

`--format json` is the shape for a program, `--format text` for a person, `--format markdown` for
pasting where somebody is talking. `--out <path>` writes the document itself rather than
redirecting the terminal, which matters on Windows: PowerShell 5.1 writes UTF-16LE with a
byte-order mark through `>`, and `json.load`, `jq`, `pandas.read_json` and `jsonlite` all reject
it.

Refusals go to stderr on every channel, so a result redirected to a file still leaves the reason
one is absent where somebody can see it.

## Exit codes

`0` means nothing declined. Anything else means something did, and a run can still carry the
quantities that were reached: a rule that found nothing leaves its own numbers absent, names
itself in the refusals, and the status reports the class. Several refusals share a status, so a
script branches on the code from `--format json` rather than on the status:

| status | meaning | codes that use it |
|---|---|---|
| 64 | the request asked for something not on offer | `command_line_not_parsed`, `decision_not_made`, `required_parameter_unstated`, `conventions_not_comparable` |
| 65 | the recording did not hold what the rule looks for | `no_crossing`, `trace_too_short`, and others |
| 66 | the file named could not be read at all | `file_not_read` |
| 78 | the registry does not load | `registry_invalid` |

`decision_not_made` and `command_line_not_parsed` are the pair worth keeping straight. The first
means a choice on your path is still open and the refusal names the values. The second means the
line itself did not parse. `plateforce capability --format json` publishes every code with its
status.

## Checks that hold this page's claims

Every command line printed in the program's own help is executed by
`scripts/check-help-examples-run.sh`, which reads them out of the help output rather than out of
the source that writes it. `scripts/check-generated-help-artefacts.sh` puts the manual pages to
`man` and the bash completion to bash, because a page that exists and a page `man` renders are
different claims.
