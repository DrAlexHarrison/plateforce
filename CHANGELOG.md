# Changelog

What changed for somebody using plateforce, one section per release.

## 0.1.2, 2026-08-14

```
plateforce version                0.1.2
method registry revision          2026-07-25
method registry digest            content-e613e95011150591
```

The registry is unchanged from 0.1.0, so the digest is the same and every number this release
returns is the number 0.1.1 returned. Nothing below changes a computed value.

### Fixed

- The desktop application offered to install itself. The window loads the same page the browser
  does, so its header carried the link that takes a reader in a tab to the download page, where
  it reaches nothing at all. It is gone in the application and unchanged in a browser.
- A folder run reported two true counts as though one contradicted the other. The summary counted
  trials and the table showed a refusal code per quantity, so a run reading "6 of 6 trials
  analysed, 0 of 6 trials declined" sat above five rows marked `dependency_unresolved` in the
  colour of a failure. The counts over trials and over quantities now sit together, each with its
  own denominator, and the failure colour marks a trial that produced nothing rather than one that
  answered nine quantities and declined two. A column no trial in the run carries is no longer
  drawn.
- `capability` did not report `markdown` among the containers a surface writes, though every
  command writes it and the guides recommend it for pasting a record where somebody is talking.
  An agent reading that array to decide what it could offer was told the format did not exist.
- Four commands answered a request for markdown with something else: `registry census` and
  `registry validate` printed text, `capability` printed JSON, and `batch` printed a text table
  and made its output folder first. Each now declines by name, as the other ten already did.

### The guides

Every quick start was rebuilt. The commands in the Windows guide were written with the line
continuation of a Unix shell and could not run in the PowerShell the same guide opens in; they
are single lines now, which run in every shell the guides name. The four guides written for a
terminal carry the version, the registry revision, the licence and the source, as the printed
guides always have, and each says on its first line which platform it is for.

## 0.1.1, 2026-08-14

```
plateforce version                0.1.1
method registry revision          2026-07-25
method registry digest            content-e613e95011150591
```

The registry is unchanged from 0.1.0, so the digest is the same and a number this release
returns differs only where a fix below says so.

### Fixed

- The drop-jump height under `jumpheight.dj.mcmahon_correction_factor` read its standing
  period over one sample more than the weighing window its own record names. The height now
  comes from exactly the declared window. On a swaying standing period the value moves by
  0.16 to 4.8 micrometres, measured at three sway amplitudes; the visible change is that a
  weighing window ending at the recording's last sample used to make the rule decline, and
  weighing a drop jump over the standing period the athlete holds at the end now answers.
- A rule handed no interval to search reported a quantity with no value and no reason: two
  landmarks enclosing no samples, a propulsive peak standing at the onset, or a weighing
  window ending at the recording's last sample. Each of those declines now and names the
  interval it was given. Measured across a sweep of the committed recordings, 27 of 675
  analyses were silent before and every one now states its reason.
- The macOS command line binaries are notarised. A copy that arrives with a quarantine flag,
  a browser download or an AirDrop, now opens cleanly; the disk image already did.

### The download page

The site root now offers the download built for the machine reading it, with the version and
size read from the newest release as the page loads, and a prompt for an AI assistant beside
the steps. The browser application lives at `/app/`, one link away, and links back.

## 0.1.0, 2026-08-14

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
quantities it does not reach.

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
