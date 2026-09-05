---
title: week
description: The Monday brief - the focus, what ships on Friday, and what waits on you.
---

```
rigger week [--week <WEEK>] [--json]
```

One screen the week opens on. It answers the three questions a Monday starts with - what am I meant to be working on, what goes out on Friday, and what is waiting on me - and answers them together, before the week is spent rather than after.

None of the three is new. `week` is the brief that arrives without being assembled from [`next`](/rigger/reference/next/), [`release-day`](/rigger/reference/release-day/) and [`inbox`](/rigger/reference/inbox/) by hand.

```console
$ rigger week --week 2026-W37
2026-W37 — 2026-09-07 to 2026-09-11

Focus
  sample [A]  v0.3.0 · Third

Ships on 2026-09-11
  sample  v0.3.0 · Third
  3 releases already out — 2 past this week's one slot
  see the queue with: rigger release-day

Waiting on you
  1 question in 1 project
  see them with: rigger inbox
```

The heading names the Monday and the Friday, because a week number is not a date anyone can picture.

## Focus

The versions [aimed](/rigger/reference/version/) at this week that have no tag yet, in tier order. A week nobody planned says so rather than showing an empty heading.

## Ships on Friday

The same queue [`release-day`](/rigger/reference/release-day/) reads, cut to what the brief needs: what is due, and whether the week's one slot on the shopfront has already been spent. The rule behind it is that a release goes out on a Friday and a version ready on a Tuesday waits - two releases in a day read as one burst from outside, two in different weeks read as a rhythm.

## Waiting on you

A count of the open questions, plus the groups where one answer settles several projects. The questions themselves are one command away; the brief carries the number so that a queue growing quietly is visible on a Monday rather than a month later.

## Their tier asks for more

The last section, present only when there is something in it. A tier is a promise about pace, and each tier breaks it in its own way:

| Tier | The promise | The signal |
| --- | --- | --- |
| A | not more than one cycle of its rhythm missed in a row | `more than one cycle missed` |
| B | a turn in the focus at least every six weeks | `no turn in the focus for N weeks` |
| C | the second declared product waits for the first to ship | `started before <project> shipped anything` |

These are floors, not paces. A carrying product that is late by its rhythm shows up under `Behind their rhythm` in [`next`](/rigger/reference/next/) at the first week over; the signal here fires only once the tier's whole allowance is spent.

Tier B is measured by the last turn rather than the last tag - a commit or a note counts. A week spent on a product that shipped nothing was still spent, and measuring by releases alone would call a worked-on product neglected.

Tier C is the one rule about a pair rather than a clock: when two declared products have both been started and neither has shipped, the later start is named along with the one it should have waited for.

## Related

- [`release-day`](/rigger/reference/release-day/) - the queue in full.
- [`next`](/rigger/reference/next/) - the same week with what is past its date.
- [`inbox`](/rigger/reference/inbox/) - the questions themselves.
- [`digest`](/rigger/reference/digest/) - what moved, once the week is over.
