//! What every help page shows below its options.
//!
//! One home for the lines a reader copies, because each of them is a claim that a command
//! runs and `scripts/check-help-examples-run.sh` runs every one of them. The check reads the
//! help output rather than this file, so a line that stops running is caught wherever it is
//! shown and a line shown nowhere is not checked into permanence by accident.
//!
//! Every example line opens with exactly two spaces and the program's name, which is the shape
//! the check extracts. Prose around them is indented differently or not at all.
//!
//! The two files an example reads, `jump.txt` and `trials/`, are the names the check creates
//! before it runs anything, so the lines here read as a reader's own folder rather than as
//! paths inside this repository.

/// The pipeline every example binds. A published one rather than a rule per step, because the
/// short form is what a reader tries first and the long form is three flags and three values
/// wide. `plateforce methods` is where the long form's names come from.
pub const TOP_SHORT: &str = "\
Examples:
  plateforce analyse jump.txt --column 0 --sample-rate-hz 1000 --sentinel none --preset sams
  plateforce methods --slot onset

`plateforce help <command>` prints one command in full, and `--help` says more than `-h`.";

pub const TOP_LONG: &str = "\
Examples:
  plateforce analyse jump.txt --column 0 --sample-rate-hz 1000 --sentinel none --preset sams
  plateforce batch trials --out-dir results --trial-suffix .force.txt --column 0 --sample-rate-hz 1000 --sentinel none --preset sams
  plateforce spread jump.txt --column 0 --sample-rate-hz 1000 --sentinel none --preset sams
  plateforce methods --slot onset
  plateforce registry show onset.threshold.noise_relative

Naming no rule at all is answered with the choices open on the path to a jump height, the
rules under each, and the values each rule was published at:

  plateforce analyse jump.txt --column 0 --sample-rate-hz 1000 --sentinel none

Where to look next:
  plateforce methods
  plateforce reach
  plateforce registry census
  plateforce capability --format json

The last is the whole surface as one document, every operation, rule, value and refusal
code, which is what a program reads instead of this page. `plateforce man` writes the
manual pages, and `plateforce completions bash` the script a shell reads.";

pub const ANALYSE_SHORT: &str = "\
Examples:
  plateforce analyse jump.txt --column 0 --sample-rate-hz 1000 --sentinel none --preset sams
  plateforce analyse jump.txt --column 0 --sample-rate-hz 1000 --sentinel none";

pub const ANALYSE_LONG: &str = "\
A published pipeline binds every rule and every value its source stated:

  plateforce analyse jump.txt --column 0 --sample-rate-hz 1000 --sentinel none --preset sams

Naming the rules one at a time takes a value for each, because the literature published
more than one and this program will not pick between them:

  plateforce analyse jump.txt --column 0 --sample-rate-hz 1000 --sentinel none --weighing bwepoch.fixed_window --set weighing.duration=1.0 --onset onset.threshold.noise_relative --set onset.k=5 --takeoff takeoff.threshold.absolute_force --set takeoff.threshold_n=20

Naming none of them is answered with the open choices, the rules under each, and the
values the literature published them at:

  plateforce analyse jump.txt --column 0 --sample-rate-hz 1000 --sentinel none

Every value each rule read, and the account each number gives of itself:

  plateforce analyse jump.txt --column 0 --sample-rate-hz 1000 --sentinel none --preset sams --provenance

As one object, for a program:

  plateforce analyse jump.txt --column 0 --sample-rate-hz 1000 --sentinel none --preset sams --format json

`plateforce methods --slot onset` names the rules a step takes, and
`plateforce registry show <METHOD>` prints one in full with every value it accepts.";

pub const BATCH_SHORT: &str = "\
Examples:
  plateforce batch trials --out-dir results --trial-suffix .force.txt --column 0 --sample-rate-hz 1000 --sentinel none --preset sams";

pub const BATCH_LONG: &str = "\
One row per trial, with the tables and the record written to --out-dir:

  plateforce batch trials --out-dir results --trial-suffix .force.txt --column 0 --sample-rate-hz 1000 --sentinel none --preset sams

A name pattern gives the run an athlete as well, which is what groups the rows:

  plateforce batch trials --out-dir results --trial-suffix .force.txt --column 0 --sample-rate-hz 1000 --sentinel none --preset sams --pattern AT{subject}_{trial}

Several rules for one quantity, one row per trial per rule:

  plateforce batch trials --out-dir results --trial-suffix .force.txt --column 0 --sample-rate-hz 1000 --sentinel none --preset sams --mode compare --against takeoff.threshold.longest_run

Analyse mode writes:
  results.csv       one row per trial
  descriptions.csv  one row per trial per quantity
  provenance.csv    one row per method per parameter
  refusals.csv      one row per declined quantity
  warnings.csv      one row per warning
  exclusions.csv    one row per gate finding
  signals.csv       one row per quality signal
  run.json          the registry digest and run fingerprint
  aggregates.csv    one row per group per quantity, when a reduction was bound

Compare mode writes:
  paired.csv         one row per trial per rule
  provenance.csv     one row per method per parameter
  refusals.csv       one row per refusal
  compare-run.json   the registry digest, base request digest and sweep

Keep each table with its run record. The record names the rules behind its numbers.";

pub const SPREAD_SHORT: &str = "\
Examples:
  plateforce spread jump.txt --column 0 --sample-rate-hz 1000 --sentinel none --preset sams";

pub const SPREAD_LONG: &str = "\
How far the choice of rule moves the jump height on this trial:

  plateforce spread jump.txt --column 0 --sample-rate-hz 1000 --sentinel none --preset sams

One quantity other than the jump height, over every rule on its path:

  plateforce spread jump.txt --column 0 --sample-rate-hz 1000 --sentinel none --preset sams --quantity takeoff_velocity_meters_per_second

A value inside a rule rather than the choice of rule, swept beside it:

  plateforce spread jump.txt --column 0 --sample-rate-hz 1000 --sentinel none --preset sams --vary onset.k=2,5,8";

pub const REGISTRY_SHORT: &str = "\
Examples:
  plateforce registry census
  plateforce registry show onset.threshold.noise_relative";

pub const REGISTRY_LONG: &str = "\
What the registry holds, each population on its own denominator:

  plateforce registry census

One rule in full: its own words, its citations, every parameter, and every value each
parameter takes:

  plateforce registry show onset.threshold.noise_relative

Whether a registry loads, and every rule violation in it:

  plateforce registry validate

`--registry <DIR>` reads a directory rather than the registry compiled in, and reaches
every command. `plateforce methods` names the rules the method flags accept.";

/// A leaf command carries its own example rather than leaning on its parent's, because
/// `plateforce registry show --help` is a page a reader reaches directly and a page that sends
/// them back up a level to find out how to call it has not answered them.
pub const CENSUS_SHORT: &str = "\
Examples:
  plateforce registry census
  plateforce registry census --format json";

pub const VALIDATE_SHORT: &str = "\
Examples:
  plateforce registry validate";

pub const SHOW_SHORT: &str = "\
Examples:
  plateforce registry show onset.threshold.noise_relative
  plateforce registry show bwepoch.fixed_window --format json

`plateforce methods` names the ids the method flags accept, and `plateforce registry census`
counts everything the registry carries.";

pub const PLATE_SAVE_SHORT: &str = "\
Examples:
  plateforce plate save lab-one --acquisition tare_state=zeroed_before_trial --acquisition floor_surface=concrete

The run names the members the plate still lacks, and a block short of any of them
fingerprints as incomplete rather than as matching.";

pub const PLATE_LIST_SHORT: &str = "\
Examples:
  plateforce plate list";

pub const PLATE_SHOW_SHORT: &str = "\
Examples:
  plateforce plate show lab-one";

pub const PLATE_FORGET_SHORT: &str = "\
Examples:
  plateforce plate forget lab-one";

pub const METHODS_SHORT: &str = "\
Examples:
  plateforce methods --slot onset
  plateforce methods";

pub const METHODS_LONG: &str = "\
The rules one step takes, which are the values --onset accepts:

  plateforce methods --slot onset

Every step, and every rule under it:

  plateforce methods

The same set as data:

  plateforce methods --format json

The three landmark steps are reached by a flag of their own, --weighing, --onset and
--takeoff. Every other step is reached by construct, through --derive or --condition, and
the heading over each block is the text to write.";

pub const REACH_SHORT: &str = "\
Examples:
  plateforce reach";

pub const REACH_LONG: &str = "\
Every construct this registry describes, split by whether a recording can reach it:

  plateforce reach

A construct out of reach carries the barrier between it and a recording: a movement
nobody recorded, an instrument nobody owns, or a rule nobody obtained. Those are facts
about an operator's movements and instruments, so a row naming one names something they
can act on.

  plateforce reach --format json";

pub const CAPABILITY_SHORT: &str = "\
Examples:
  plateforce capability --format json";

pub const CAPABILITY_LONG: &str = "\
One document carrying every operation this surface dispatches, every rule it runs with the
slot each fills, every value each rule takes with the exact text that states it, the
acquisition block's members, the containers this surface writes, and every refusal code
with its exit code:

  plateforce capability --format json

`--registry <DIR>` reaches this command as it reaches every other, and the values reported
are the ones that registry publishes.

For a reader rather than a program, `plateforce methods` prints the rules under the words
that reach them, and `plateforce registry show <METHOD>` prints one in full.";

pub const PLATE_SHORT: &str = "\
Examples:
  plateforce plate list";

pub const PLATE_LONG: &str = "\
Every plate saved on this machine:

  plateforce plate list

Recording one, so a later run is told about it by name:

  plateforce plate save lab-one --acquisition tare_state=zeroed_before_trial --acquisition floor_surface=concrete

Reading one back:

  plateforce plate show lab-one

`--plate <NAME>` on analyse and batch fills the acquisition block from a saved plate. A
block short of any member fingerprints as incomplete rather than as matching, and
`plateforce plate save` names the members it still lacks.";

pub const MAN_SHORT: &str = "\
Examples:
  plateforce man";

pub const MAN_LONG: &str = "\
Writes one page per command, beside this machine's other manual pages:

  plateforce man

Writes them somewhere else:

  plateforce man --out-dir pages

The pages go in a man1 directory inside the one named, which is where `man` looks. The
run prints both the path and the command that reads a page from it.";

pub const COMPLETIONS_SHORT: &str = "\
Examples:
  plateforce completions bash";

pub const COMPLETIONS_LONG: &str = "\
Writes the script to the terminal, which a shell reads directly:

  plateforce completions bash

  plateforce completions zsh

Writes it to a file under the name that shell looks for, and prints the path:

  plateforce completions zsh --out-dir completions

For the current bash shell:
  source <(plateforce completions bash)

For the current zsh shell, initialise completions before loading its script:
  autoload -Uz compinit && compinit
  source <(plateforce completions zsh)

Kept between sessions, the script goes in a directory the shell reads at startup:
~/.local/share/bash-completion/completions for bash, a directory on fpath for zsh,
~/.config/fish/completions for fish.";

pub const VERSION_SHORT: &str = "\
Examples:
  plateforce version
  plateforce version --format json";
