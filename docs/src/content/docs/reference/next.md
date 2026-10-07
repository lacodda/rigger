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

## The JSON is a contract

`next --json` is what a release engine reads to learn what the week is aiming at - with [`version show --json`](/rigger/reference/version/#the-json-is-a-contract), it is how `furca release plan` proposes a number and a theme from the record rather than from commits alone. Every field is named here, and the test suite fails when the JSON prints one this page does not name.

<!-- json: next -->
| Field | Type | Meaning |
| --- | --- | --- |
| `week` | string | The week read, as `2026-W39`. |
| `friday` | string | Its Friday, as `2026-09-25`. |
| `focus` | array | The versions aimed at this week with no tag yet. |
| `focus[]` | object | A version aimed at the week that has no tag yet. |
| `focus[].project` | string | The project. |
| `focus[].tier` | string or null | Its tier: `A`, `B`, `C` or `out`; `null` when it has none. |
| `focus[].version` | string | The version's number. |
| `focus[].title` | string or null | The stage's title, or `null`. |
| `focus[].planned` | string | The week it was aimed at, as `2026-W41`. |
| `focus[].overdue_weeks` | integer or null | Always `null` in `focus`. |
| `overdue` | array | The versions aimed at a week already gone, with no tag. |
| `overdue[]` | object | A version aimed at a week already gone, with no tag. |
| `overdue[].project` | string | The project. |
| `overdue[].tier` | string or null | Its tier: `A`, `B`, `C` or `out`; `null` when it has none. |
| `overdue[].version` | string | The version's number. |
| `overdue[].title` | string or null | The stage's title, or `null`. |
| `overdue[].planned` | string | The week it was aimed at, as `2026-W41`. |
| `overdue[].overdue_weeks` | integer | How many weeks that week is past. |
| `lapsed` | array | The projects behind the rhythm of their tier. |
| `lapsed[]` | object | A project behind the rhythm its tier asks for. |
| `lapsed[].project` | string | The project. |
| `lapsed[].tier` | string | Its tier: `A`, `B` or `C`. |
| `lapsed[].rhythm_weeks` | integer | How many weeks between releases its tier asks for. |
| `lapsed[].since` | string or null | The week of its last release, as `2026-W41`; `null` if it never shipped. |
| `lapsed[].weeks` | integer | How many weeks it has gone without a release. |
| `signals` | array | The projects whose tier asks for more. |
| `signals[]` | object | A project whose tier asks for more than it gives. |
| `signals[].project` | string | The project. |
| `signals[].tier` | string | Its tier: `A`, `B` or `C`. |
| `signals[].signal` | string | The promise broken: `missed-cycle` (tier A), `without-focus` (tier B) or `second-start` (tier C). |
| `signals[].weeks` | integer or null | Weeks behind, for `missed-cycle` and `without-focus`; otherwise `null`. |
| `signals[].alongside` | string or null | The other project, for `second-start`; otherwise `null`. |
| `parted` | array | The pairs whose halves have parted company. |
| `parted[]` | object | A pair of linked projects whose halves have parted company. |
| `parted[].drift` | string | How they parted: `shipped-alone` (one half is tagged, the other is not) or `run-ahead` (one end moved on by several versions while the other stood still). |
| `parted[].ahead` | object | The end that went ahead, or shipped. |
| `parted[].ahead.project` | string | The project at that end. |
| `parted[].ahead.version` | string or null | The version anchored on that side, or `null` when none is. |
| `parted[].ahead.shipped` | boolean or null | Whether that version has a tag; `null` when no version is anchored. |
| `parted[].behind` | object | The end left behind. |
| `parted[].behind.project` | string | The project at that end. |
| `parted[].behind.version` | string or null | The version anchored on that side, or `null` when none is. |
| `parted[].behind.shipped` | boolean or null | Whether that version has a tag; `null` when no version is anchored. |
| `parted[].versions` | integer or null | How many versions apart the ends are, for `run-ahead`; otherwise `null`. |
| `parted[].note` | string or null | What the pair shares, as the link records it, or `null`. |

Fields may be added; none is renamed or removed without a major version.

## Related

- [`week`](/rigger/reference/week/) - the same week as a brief, with what waits on you.
- [`release-day`](/rigger/reference/release-day/) - the shopfront queue for the week.
- [`calendar`](/rigger/reference/calendar/) - the same facts across several weeks.
- [`version plan`](/rigger/reference/version/) - what puts a version in a week.
- [`project tier`](/rigger/reference/project/) - what sets the rhythm this checks.
- [`inbox`](/rigger/reference/inbox/) - the other thing waiting on you.
