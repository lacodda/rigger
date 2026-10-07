---
title: release-day
description: The shopfront queue - what has gone out this week, and what waits for Friday.
---

```
rigger release-day [--week <WEEK>] [--json]
```

The week as the outside sees it. The rule it reads against is the one the line set for its shopfront: **a release goes out on a Friday, and a version ready on a Tuesday waits in the queue.**

The reason is not tidiness. Two releases on one day read as a single burst to anyone watching; two in different weeks read as a rhythm. What is meant to be even is the trace, not the effort - the work goes in bursts and always will.

```console
$ rigger release-day --week 2026-W37
2026-W37 — releases on 2026-09-11

Waiting for Friday:
  sample  v0.3.0 · Third

Already out this week:
  2026-09-08  early    1  sample v0.1.0
  2026-09-09  early    1  sample v0.2.0
  2026-09-11  Friday   1  widget v0.1.0

2 releases past the one release this week has room for
2 releases went out before Friday
```

## Waiting for Friday

Versions [aimed](/rigger/reference/version/) at this week whose tag has not been made yet. This is the queue proper: what the Friday is for.

## Already out this week

What the tags say has shipped, folded by day. Each line is one day: the date, whether it is the week's Friday, how many releases it carried, and which projects they came from.

A project's versions are named while they are few and spanned once they are not - `v0.1.0..v0.12.0 (12)`. The fold is not cosmetic. A real week of this line holds ninety-four releases, and a line each would put the two numbers that answer the question below the fold, where nobody reading a terminal would find them.

## The two numbers

The last lines say which half of the rule is being broken, and they are counts rather than complaints - the record reports, and what to do about it is yours.

- **past the one release this week has room for** - the week carried more than one release, so the trace shows a burst rather than a beat.
- **went out before Friday** - the tag was made earlier in the week, so the release did not wait its turn.

A week that kept the rule shows neither line.

## `--json`

The same facts as data. [`week`'s](/rigger/reference/week/) `shipping` field reads the same versions, so the two screens cannot disagree about a week.

<!-- json: release-day -->
| Field | Type | Meaning |
| --- | --- | --- |
| `week` | string | The week read, as `2026-W37`. |
| `friday` | string | Its Friday, as `2026-09-11`. |
| `early` | integer | How many releases went out before Friday. |
| `over_the_slot` | integer | How many releases the week carried past its one slot. |
| `shipped` | array | The versions whose tag is already in the week. |
| `shipped[]` | object | A version whose tag is in the week. |
| `shipped[].project` | string | The project. |
| `shipped[].version` | string | The version's number. |
| `shipped[].day` | string | The day of the tag, as `YYYY-MM-DD`. |
| `shipped[].on_release_day` | boolean | Whether the tag landed on the Friday the week releases on. |
| `queued` | array | The versions aimed at this week with no tag yet: the queue proper. |
| `queued[]` | object | One queued version. |
| `queued[].project` | string | The project. |
| `queued[].version` | string | The version's number. |
| `queued[].title` | string or null | The stage's title, or `null`. |

## Related

- [`week`](/rigger/reference/week/) - the Monday brief, of which this is one section.
- [`calendar`](/rigger/reference/calendar/) - several weeks at once, as a grid.
- [`version plan`](/rigger/reference/version/) - what puts a version in the queue.
- [`sync`](/rigger/reference/sync/) - what reads the tags this counts.
