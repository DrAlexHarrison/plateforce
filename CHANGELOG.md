# Changelog

What changed for somebody using plateforce, one section per release.

## 0.1.0, 2026-08-13

First release.

### What identifies the numbers you get

Every answer carries three things that together say exactly which software and which rule set
produced it. Record them beside any number you report, and quote them in a methods section:

```
plateforce version                0.1.0
method registry revision          2026-07-25
method registry digest            content-e613e95011150591
```

The revision is the date the registry names itself. The digest is taken over the registry's
contents, so it changes when any rule, parameter or citation changes, whether or not the
revision moved. Two results agree on how they were computed when both of those agree.

Every result carries all three. In JSON they are `plateforce_version`,
`registry_declared_version` and `registry_digest`. A fourth field, `registry_version`, is
empty until you pin a revision yourself, and holds the one you asked for when you do.

This matters more here than the version number alone would suggest. Ten published ways of
computing one jump height disagree by a median of 3.51 cm on 244 real trials, against the
1.98 cm training effect the study behind those trials was built to detect. A later release
can return a different number from the same file because a rule changed, and these three
identifiers are what let you tell that apart from a change in the athlete.

### What it does

Reads a vertical ground reaction force trace and computes the quantities a jump study
reports. Among them: system weight, movement onset, takeoff, time to takeoff, flight time,
takeoff velocity, net impulse, jump height in the standing and takeoff frames, power, work,
rate of force development, reactive strength, and the braking, propulsion and landing phase
boundaries. `plateforce reach` prints the whole set, and says what stands in the way of the
quantities it does not reach on the recording you gave it.

Every value comes back with the rule that produced it, the parameters that rule was given,
and where each of those values came from: stated by you, measured off the recording, or
supplied by the rule.

Where published methods disagree and nothing in your request settles the choice, it says so
and computes nothing until you choose. `--preset` answers every choice a published pipeline
states, out of one source.

`spread` sweeps a quantity over every rule on its path and reports how far the method choice
moves it, on your own trial rather than on a figure borrowed from a paper.

`batch` runs a folder and writes one row per trial, with a long-form table you can filter a
cohort question over, a provenance table that joins back to it, and a `run.json` carrying the
three identifiers above. Counts come with their denominators.

### Where it runs

The same computation, compiled once and bound to each surface, so a number does not depend on
which one you used:

- A browser tab. The file is read on your machine and nothing is uploaded.
- A desktop application on Linux, macOS and Windows, with no compiler and no package manager.
- A terminal, on every common shell, with manual pages and shell completions.
- R and Python.

### What the software will tell you about itself

Ask it rather than reading a document that goes stale:

```
plateforce capability          what this build can do, as JSON
plateforce methods             the rules it runs, under the words that reach them
plateforce registry census     the registry's populations, each on its own denominator
plateforce reach               what it can compute, and what stands in the way of the rest
```

### Licence

Apache-2.0. See `LICENSE` and `NOTICE`.
