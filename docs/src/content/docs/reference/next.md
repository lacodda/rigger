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

## Related

- [`calendar`](/rigger/reference/calendar/) - the same facts across several weeks.
- [`version plan`](/rigger/reference/version/) - what puts a version in a week.
- [`project tier`](/rigger/reference/project/) - what sets the rhythm this checks.
- [`inbox`](/rigger/reference/inbox/) - the other thing waiting on you.
