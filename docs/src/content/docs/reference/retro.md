---
title: retro
description: The look back - what the plan said, what the tags say, and where the two parted company.
---

```
rigger retro [--cycle | --weeks <N>] [--to <WEEK>] [--record] [--json]
```

A window of the record read backwards. [`calendar`](/rigger/reference/calendar/) and [`next`](/rigger/reference/next/) ask what is coming; this asks what happened, and whether it was what anyone said would happen.

Nothing here is a new fact. Every number comes from two things already recorded: the week a version was [aimed at](/rigger/reference/version/), and the tag [`sync`](/rigger/reference/sync/) read. The check is worth having because it is the one the written calendar asked for every seven weeks and had no way to perform - nothing there ever read a tag, so the check was a thing to remember, and a thing to remember is a thing that stops happening.

```console
$ rigger retro --cycle
2026-W34 to 2026-W40 — 7 weeks

4 shipped — 1 on time, 1 slipped, 2 unplanned
50% of what shipped had been planned

Planned and not shipped:
  sample  v0.3.0 — was due 2026-W39 (a week late by the end of the window)

Shipped, but not when it was aimed:
  sample  v0.2.0 — aimed at 2026-W37, out in 2026-W39 (2 weeks late)

Per project:
  sample [A]  2 shipped (2 planned), 3 asked, 1 missed
  widget [C]  2 shipped (0 planned), 1 asked

Shipping past their tier — it may be describing the wrong thing now:
  widget [C]  2 shipped against 1 asked for (2x)
  move one with: rigger project tier <project> <A|B|C|out>
```

## The window

`--weeks <N>` looks back over N weeks ending with this one; four by default - long enough to hold more than one release of a fast-moving product, short enough that a Monday can read it. `--cycle` uses seven, the cycle length the release calendar was written around: two turns for a carrying product and one for everything else.

`--to <WEEK>` ends the window somewhere other than the current week, which is how a past cycle is read.

## The three numbers

Every release inside the window is exactly one of three things, and the counts add up to the total:

- **on time** - the tag landed in the week the version was aimed at;
- **slipped** - it was aimed somewhere and landed somewhere else;
- **unplanned** - nobody ever aimed it anywhere.

Under them is the share of what shipped that had been planned at all. This is the number a written calendar can never produce, and the one that says whether planning is happening: a line where every release is unplanned has a calendar in name only. It truncates rather than rounds - the one direction this figure must never err in is upwards.

## Planned and not shipped

A version aimed at a week inside the window with no tag anywhere. It is late **against the end of the window**, not against today: a look back has to give the same answer whenever it is run, and measuring against the clock would grow the number every time the same cycle was read.

## Do the tiers still fit

"Whether the tiers need moving" was the third question the written calendar asked. The answer is a comparison held up for you, not a decision taken for you - and the two directions are shown apart, because they are different problems:

- **Nothing shipped, and their tier asked for something.** A product that has stalled. This list comes first: it is the one that needs a decision.
- **Shipping past their tier.** A product that has outgrown the tier rather than fallen behind it, shown once it reaches twice the asked-for pace.

Keeping them in one list of "misfits" was the first thing the real record disproved: seven of fifteen projects came back as misfits, and the two that had stalled sat buried among five that were racing.

A project with no tier, or one set to `out`, appears in neither. Nothing was promised for it, so there is nothing to fall short of.

## `--record`

Keeps the summary in the record, so that a later look back can find what this one concluded. A retro that is only ever printed leaves the same hole the written calendar had, where the check happened and nothing afterwards could tell that it did.

The summary belongs to no single project, because it is about all of them - so it goes to a place the record keeps for itself, which is made once:

```console
$ rigger project service line
Recorded 'line' as a place the record keeps for itself
  no repository: sync will not ask git about it
```

Without one, `--record` says so and names the fix rather than filing the summary somewhere misleading.

The event is dated by the **window** it looked at, not by the moment it ran, so asking for the same retro twice keeps it once. A window where nothing shipped and nothing was planned is not kept at all: a retro is filed so a later one can find a conclusion, and "nothing" concludes nothing.

## Related

- [`calendar`](/rigger/reference/calendar/) - the same facts looking forward, as a grid.
- [`week`](/rigger/reference/week/) - one week as a brief.
- [`release-day`](/rigger/reference/release-day/) - one week as the shopfront sees it.
- [`project tier`](/rigger/reference/project/) - what this compares against, and what moves it.
