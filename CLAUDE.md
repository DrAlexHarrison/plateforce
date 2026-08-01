# plateforce

Force-plate analysis where every result carries the method that produced it.

## Before anything else

Read `/home/alex/force-plate-lab/ORIENTATION.md` in full. It is short, and it is the only file
that says which of thirty documents to believe, what Alex has already ruled, what has already
been measured, and how this project characteristically goes wrong. Sessions that skip it
propose research finished in July and re-decide questions Alex settled in his own words.

## The one principle

Published methods for the same jump metric disagree enough that the choice of method moves
the number further than a training intervention does. Across all seven competing open tools,
grepping for `fingerprint`, `provenance` or `method_id` returns zero hits. So the product is
not the maths. The product is the record of what produced each number.

**A silent default or an unrecorded choice inside plateforce outranks any missing feature.**
It is the disease reproduced in the cure. Weight those findings above feature work, every
time.

## What done means

A robust, easy-to-use tool for one jump trace or a batch, computing every variable the
industry uses, on any operating system, through a GUI or a terminal, operator's choice.

The registry enumerates the scope, and the count is a query rather than a line in this file:
`cargo run -q -p plateforce-cli -- registry census`. Done is measurable against it. Full
definition and the route there: `/home/alex/force-plate-lab/MISSION.md`, then
`/home/alex/force-plate-lab/phase2/ROADMAP.md`.

## Binding on every file and every agent

- `CONVENTIONS.md` in this repo. Section 3 (the maths has one home, methods are data) and
  section 5 (interface copy describes the user's data, never the state of the software).
- `/home/alex/rc/.claude/rules/writing.md`.
- No em dashes. Commas, colons, periods.
- Descriptive names carrying units. Comment density one line per 10 to 30 lines.

**Never write a comment or a doc note that records a limitation in place of fixing it.** A
limitation written into the source is not honesty, it is the work not done. Fix it, or raise
it in chat. Do not commit it as prose.

## Constraints that do not bend

- Only subject 01 (Michelle) data is ever public, derived data included. No other athlete
  data, ever.
- Adding a method is a data edit. A rule's text belongs in the registry, never in a function
  body.
- Every count is reported with its denominator, and the two registry populations are never
  summed.

## Verifying

Clippy caches per crate and under-reports straight after a build; force a full run. Checking
an exit code through a pipe reports the pipe's status. Verify a push with
`git merge-base --is-ancestor` rather than reading piped output.
