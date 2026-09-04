---
title: calendar
description: Weeks by projects - what is planned, what shipped, what slipped.
---

```
rigger calendar [--weeks <N>] [--from <WEEK>] [--json]
```

The release calendar as a grid: weeks across, projects down. What a version was aimed at comes from [`version plan`](/rigger/reference/version/); what actually happened comes from the tags [`sync`](/rigger/reference/sync/) reads. Nothing here is a status anyone sets - slippage is the difference between the two, and it appears the moment a tag does.

```console
$ rigger calendar --from 2026-W37 --weeks 5
         2026-W37  2026-W38  2026-W39  2026-W40  2026-W41
sample   *v0.1.0             >v0.2.0             ·v0.3.0    A
widget             +v0.1.0   ·v0.2.0                        B

+ shipped as planned   > slipped   ! overdue   * unplanned   · planned

sample   v0.2.0 — aimed at 2026-W37, 2 weeks late
```

The trailing column is the project's tier, when it has one.

## The marks

| Mark | Meaning |
| --- | --- |
| `+` | shipped in the week it was aimed at |
| `>` | shipped, but not in that week |
| `!` | the planned week has passed and there is no tag |
| `*` | shipped without ever being planned |
| `·` | planned, still to come |

A release that slipped is shown **in the week its tag is in**, not the week it was aimed at. Showing it in its planned week would leave the grid claiming a release happened in a week that has none; the aim is not lost, it is named underneath with the distance in weeks.

A version that was never aimed at a week still appears once it ships. Hiding it would let the calendar disagree with the tags, which is the failure it exists to prevent.

## Busy weeks

Two releases in a week are named; more than two are counted, as a span:

```console
$ rigger calendar --from 2026-W33 --weeks 3
             2026-W33               2026-W34                2026-W35
sample       *v0.1.0..v0.14.0 (14)  *v0.15.0 *v0.16.0       >v0.17.0..v0.19.0 (3)   B
```

This is not tidiness. A week of a real record can hold dozens of releases, and naming them all stretches the column past three hundred characters - every row wraps, the heading stops lining up, and the grid cannot be read at all.

The mark on a counted cell is the **worst** standing inside it, so one release that slipped into a busy week is not hidden by the ones that landed on time beside it. The names are in [`why`](/rigger/reference/why/) either way.

## Weeks

Weeks are ISO-8601: `2026-W37` is the week of Monday 7 September, and it releases on Friday 11 September. ISO is used because it is the same everywhere, needs no locale to read, and starts on the Monday this line already works to.

`--weeks` is how many to show, starting from this week; `--from` reads a different span. The current week is marked `*` in the heading, so a grid read on a Wednesday says where the reader is standing.

A version that is neither aimed at a week nor shipped is in the plan and not on the calendar - [`digest`](/rigger/reference/digest/) is the screen for that.

## Related

- [`version plan`](/rigger/reference/version/) - aim a version at a week.
- [`next`](/rigger/reference/next/) - one week of this, in full.
- [`project tier`](/rigger/reference/project/) - the tier and rhythm shown here.
- [`sync`](/rigger/reference/sync/) - what reads the tags this compares against.
