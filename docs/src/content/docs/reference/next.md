---
title: next
description: This week's focus, what is past its week, and what is behind its rhythm.
---

```
rigger next [--week <WEEK>] [--json]
```

One week of the [calendar](/rigger/reference/calendar/), read in full. What is aimed at this week, what was aimed at a week already gone, and which projects have quietly stopped releasing.

```console
$ rigger next --week 2026-W39
2026-W39 — releases on 2026-09-25

widget [B]  v0.2.0 · Second
```

The heading names the Friday, because that is the day the release is for; a week number is not a date anyone can picture.

## Past their week

A version aimed at a week that has gone by without a tag:

```console
$ rigger next --week 2026-W44
2026-W44 — releases on 2026-10-30

Nothing is aimed at this week.

Past their week:
widget  v0.2.0 — was due 2026-W39 (5 weeks ago)
sample  v0.3.0 — was due 2026-W41 (3 weeks ago)

Behind their rhythm:
sample [A]  last shipped 2026-W39, 5 weeks without a release, rhythm is 2 weeks
widget [B]  last shipped 2026-W38, 6 weeks without a release, rhythm is 4 weeks
```

A version planned for **this** week is not late. Only a week already past is - otherwise the current focus would be marked overdue on the Monday it began.

## Behind their rhythm

The second list is a different question. "Past their week" is about one release; this is about a project that has stopped releasing at the pace its tier asks for - which no written calendar can notice, because nothing there ever compares the rotation to the tags.

A project needs a tier and a rhythm to be checked. One with neither is out of the rotation by omission, and a project explicitly set to `out` is out by decision: neither is named here, whether or not a rhythm was spelt out for it. Being out of the rotation is the point of saying `out`.

A project that has never shipped counts as behind from the start: "never" and "not lately" are the same problem.

## Their tier asks for more

A third list, below the other two, present only when there is something in it. "Behind their rhythm" is about a pace; this is about the floor each tier promised not to go under, and the two fire at different moments.

| Tier | The promise | The signal |
| --- | --- | --- |
| A | not more than one cycle of its rhythm missed in a row | `more than one cycle missed` |
| B | a turn in the focus at least every six weeks | `no turn in the focus for N weeks` |
| C | the second declared product waits for the first to ship | `started before <project> shipped anything` |

```console
$ rigger next --week 2026-W44
2026-W44 — releases on 2026-10-30

Nothing is aimed at this week.

Past their week:
sample  v0.3.0 — was due 2026-W37 (7 weeks ago)

Behind their rhythm:
sample [A]  last shipped 2026-W37, 7 weeks without a release, rhythm is 2 weeks
widget [B]  last shipped 2026-W37, 7 weeks without a release, rhythm is 4 weeks

Their tier asks for more:
sample [A]  more than one cycle missed - 7 weeks without a release
widget [B]  no turn in the focus for 7 weeks
```

A carrying product is allowed one missed cycle, so it appears under "Behind their rhythm" first and here only once the allowance is spent. A growing one is measured by its last turn rather than its last tag - a commit or a note counts, because a week spent on a product that shipped nothing was still spent. The declared products are checked against each other rather than against the clock: when two have been started and neither has shipped, the later start is named with the one it should have waited for.

A project set to `out`, or with no tier at all, raises nothing.

## Related

- [`week`](/rigger/reference/week/) - the same week as a brief, with what waits on you.
- [`release-day`](/rigger/reference/release-day/) - the shopfront queue for the week.
- [`calendar`](/rigger/reference/calendar/) - the same facts across several weeks.
- [`version plan`](/rigger/reference/version/) - what puts a version in a week.
- [`project tier`](/rigger/reference/project/) - what sets the rhythm this checks.
- [`inbox`](/rigger/reference/inbox/) - the other thing waiting on you.
