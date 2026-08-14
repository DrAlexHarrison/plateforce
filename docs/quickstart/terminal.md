# Quick Start

## plateforce at a terminal

Read a force trace, get the jump numbers, and keep the record of how they were computed.

This guide is written to be run start to finish, on macOS, Windows or Linux. Every command is
written out in full, with your own file name in place of `trial.txt`. A person and an
assistant working on their behalf can both follow it.

## What it does

plateforce reads a force-plate recording and computes the numbers a jump study reports: jump
height, time to takeoff, flight time, impulse, power, and the rest. Every number it gives you
carries the id of the rule that produced it, and `plateforce registry show <id>` prints that
rule in full, with the papers behind it.

That second half is the reason to use it. Ten published ways of computing one jump height
disagree by a median of 3.5 cm on the same 244 real trials, and the training effect the study
behind those trials was built to detect was 2.0 cm. Two jump heights are only comparable when
they were computed the same way, so plateforce keeps the record that makes the comparison
possible.

<!--GET-THE-PROGRAM-->

## What plateforce needs to know about your file

Three things, and it will not guess any of them.

**Which column carries the vertical force**, counting from zero. `--column 0` is the first
column.

**How many samples per second.** Force exports rarely carry this and it cannot be read from
the numbers. Reading a 1200 Hz recording as 1000 Hz puts every velocity and impulse out by a
fifth, and every height and displacement, which go with the square of the rate, out by nearly
half.

**How the file writes a sample it does not have.** Some export software writes `0` or `-1`
where no measurement was taken. `--sentinel zero`, `--sentinel negative_one`, or
`--sentinel none` when every value in the file is a measurement.

A fourth, only when your columns are not separated by a single character: `--delimiter` takes
one character, or the word `whitespace` when the columns are held apart by runs of spaces.
Left out, each row is read whole as a single column.

## One trial

Start by asking for the analysis and letting the program tell you what is missing:

```
plateforce analyse trial.txt --column 0 --sample-rate-hz 1200 --sentinel none
```

It computes nothing and exits 64. What it prints is the point: the choices on the way to a
jump height that have no default, what the literature publishes for each, and the flag that
answers it.

```text
plateforce: 2 of 3 choices on the path to a jump height have no default.

  --weighing <METHOD>   Standing still, before the jump   system_weight
      System weight includes the bar. Bodyweight does not. The registry records
      real conflations.
      bwepoch.fixed_window                                accepted
          duration published at 0.1, 0.25, 0.4, 0.5, 1.0, 2.0
      bwepoch.adaptive_lowest_variance                    recommended
          window_seconds published at 0.2, 0.5, 1.0, 2.0
      bwepoch.manual_placement                            accepted
```

Answer them and it runs:

```
plateforce analyse trial.txt --column 0 --sample-rate-hz 1200 --sentinel none \
  --weighing bwepoch.adaptive_lowest_variance --set weighing.window_seconds=1.0 \
  --onset onset.threshold.noise_relative --set onset.k=5 \
  --takeoff takeoff.threshold.flight_noise_k_sd
```

Or take a published pipeline whole, which binds every rule and every value its source states:

```
plateforce analyse trial.txt --column 0 --sample-rate-hz 1200 --sentinel none --preset sams
```

`plateforce methods` names every rule it runs, grouped under the flag that reaches it, and
the published pipelines it carries. `plateforce methods --slot onset` narrows it to
the rules one step takes.

## Reading what comes back

Four blocks, in this order.

**Trial** names the file, its length and its rate, and states what it did with the sentinel
convention you declared.

**The numbers**, each with the rule that produced it directly underneath. A quantity this
recording cannot support reads `no value` and says which rule declined and why. Nothing is
dropped quietly.

**Method spread** puts a size on the method choice: the same quantity under every combination
of rules on its path, over your trial, with the lowest and highest named.

**Rules** and **Global to this analysis** are the record: every rule that ran with the values
it was given, and the values that applied to the whole analysis. `--provenance` adds the
values each rule chose for itself, and a section giving every number its own account of how
it was produced.

`--format json` and `--format markdown` carry all of that without being asked, and mark each
value cited, measured or assumed. JSON is the document a program reads, markdown is a block
to paste into a document. `--out <path>` writes it to a file.

## A folder of trials

```
plateforce batch trials/ --out-dir results \
  --trial-suffix .txt --column 0 --sample-rate-hz 1200 --sentinel none --preset sams
```

`--trial-suffix` is not optional and is never guessed: it is how the run says which files in
that folder are trials, and the summary reports how many files carried it and how many did
not.

`results/` then holds one table per relation:

| file | what is in it |
|---|---|
| `results.csv` | one row per trial, one column per quantity |
| `provenance.csv` | which rule and which values produced each number |
| `descriptions.csv` | the account each number gives of itself |
| `refusals.csv` | every quantity that declined, with the rule and the reason |
| `warnings.csv` | what the run wants you to look at |
| `exclusions.csv` | anything a gate removed, and under which rule |
| `signals.csv` | the landmarks per trial |
| `run.json` | the request, the registry, and the record for the whole run |

`--pattern 'AT{subject}_{trial}'` reads a subject out of the file names, which is what lets a
run reduce an athlete's trials to one number under a published rule. `trial.aggregation`
carries three such rules and none of them is the arithmetic mean, so `--aggregate` names the
one you want and `--aggregate-n` says how many trials it was asked for. Best of five and best
of three are two requests of one rule, and the count travels with the value:

```
plateforce batch trials/ --out-dir results \
  --trial-suffix .txt --column 0 --sample-rate-hz 1200 --sentinel none --preset sams \
  --pattern 'AT{subject}_{trial}' \
  --derive analysis_window=window_end.takeoff.detected \
  --derive net_peak_force=force.peak.net \
  --aggregate best_of_n_by_peak_force --aggregate-n 5 \
  --aggregate-quantity net_peak_force_newtons
```

`best_of_n_by_peak_force` orders an athlete's trials on net peak force, so the run has to
compute it. `force.peak.net` reads an analysis window, so the first `--derive` places that
window and the second computes net peak force over it. `--aggregate-quantity` says which
computed column to reduce. A run that aggregates writes a ninth table into `results/` beside
the eight above, `aggregates.csv`, one row per athlete per quantity, carrying the reduced
value, its dispersion and the requested trial count.

## If you would rather click

```
plateforce serve
```

It prints an address. Open it in a browser on that machine and the full graphical interface
is there, with the same registry and the same record. The address is bound to the machine
itself, so nothing on the network can reach it, and no request leaves the computer. This is
the route for a laboratory machine that will not let you install anything.

## For an assistant, or a script

`--format json` returns a document with a single `ok` or a single `refusal` from every command
that reports a result: `analyse`, `batch`, `spread`, `capability`, `methods`, `reach`,
`registry`, `plate` and `version`. `completions` and `man` write files for a shell and for
`man` to read, and print the paths they wrote. `serve` prints the address it is listening on.

`plateforce capability` reports every operation, method, output format and refusal code this
build reaches, as JSON. That is the one call to make before writing anything against this
program, because it describes the program in front of you rather than the one somebody wrote
about.

`plateforce methods` names every rule it runs, under the exact flag that reaches it,
and `plateforce registry show <id>` reads one of them out in full: what it does, what it is
published as, what it is known to do to a result, and what it cites.

`plateforce man` writes the manual pages for every command and prints the two commands that
read them. `plateforce completions <shell>` prints the completion script; add
`--out-dir <dir>` to write the file your shell looks for and print its path.

Exit codes follow `sysexits`, so a caller can branch without reading prose:

| code | meaning |
|---|---|
| 0 | a result was produced |
| 64 | the request asked for something that is not on offer, including a choice with no default |
| 65 | the recording did not hold what the rule looks for, and no result was produced |
| 66 | the file named could not be read |
| 69 | the port is already in use |
| 70 | the program broke an invariant it states, and no result was produced |
| 77 | the port is not one this process may open |
| 78 | the registry does not load |

**The status says whether a result reached you, not whether every number arrived.** A trial
whose recording stops before the athlete lands has no flight time, and a run that computes
everything else and records what declined exits 0. The quantities that declined are in
`refusals.csv`, in `refusals` in the JSON, and printed beside the numbers in the text. Branch
on those rather than on the status if you want to know whether anything was missing.

## When something does not work

**It refuses with a choice that has no default.** That is the program working. It is telling
you that published rules disagree here and that it will not pick for you. The refusal names
every rule it will accept.

**A rule found no crossing.** The message names the rule, the value, and the window it
searched. Usually the trial is not the movement that rule expects, or the recording was
trimmed before the event. Try another rule for that step.

**You do not know the sample rate.** Do not guess it. It is in the collection software's
settings, in the study protocol, or with whoever ran the session. A time column gets you
close enough to recognise the answer: take the last time minus the first, divide by the
number of intervals, and take one over that. Two rows are not enough, because the export
rounds each stamp. On a real 1200 Hz column, two rows 0.000833 s apart give 1200.48 Hz, and
the whole column gives 1199.99992 Hz.

**A quantity reads `no value`.** The recording does not support it, and the reason is printed
beside it. A trace that stops before the athlete lands has no flight time, and nothing that
depends on flight time can be computed from it.

## Where else it runs

The same analysis, the same registry and the same record are available in a browser tab at
[dralexharrison.github.io/plateforce/app/](https://dralexharrison.github.io/plateforce/app/), and as an
application on macOS, Windows and Linux. A result computed on one carries the record that
lets another reproduce it.
