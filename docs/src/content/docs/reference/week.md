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

A question past the day it was due is named here, not only counted - in red, for a person at a terminal:

```console
Waiting on you
  2 questions in 1 project
  sample [7] Which registry goes first?  overdue since 2026-09-21
  see them with: rigger inbox
```

## The week as a toast

The brief is a command, and a command is read by whoever runs it - on this line, mostly the assistant. So the first time rigger runs in a new week, whatever runs it, the head of the brief goes to the desktop as a notification:

```
rigger · 2026-W39 · releases on 2026-09-25
Focus: sample v0.3.0, widget v0.2.0
1 question overdue · 1 version past its week
```

The first run claims the week in one statement, so the MCP server starting on Monday morning and a command at the terminal cannot both toast. A week with nothing to say - no focus, nothing late, nothing waiting - is not toasted; a toast announcing an empty week teaches its reader to dismiss it. Nothing about the toast can fail the command it rides on.

It is shown through what the platform already has - PowerShell on Windows, `osascript` on macOS, `notify-send` elsewhere - so nothing is installed for it. `RIGGER_NOTIFY` names another program to show it: the title arrives as its one argument and the body on standard input. A record kept under `RIGGER_DATA_DIR` toasts only through a program named that way, so a test never reaches the desktop it runs on.

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

## Pairs out of step

A section of its own, after the tier signals, present only when there is something in it:

```
Pairs out of step
  kasl v1.13.0 shipped without kasl-server v0.23.0 — the machine report
```

A tier signal is about one product going too slowly. This is about two that stopped agreeing - two halves of one capability where one has shipped and the other has not, or where one has released twice past where the pair was agreed while the other stood still. The answer to one is a week of work; the answer to the other is usually a release of the half left behind.

It is here, and in [`next`](/rigger/reference/next/) and [`retro`](/rigger/reference/retro/), because a pair belongs to no single project. Every screen that reads one project at a time is the reason this kind of drift went unseen for weeks at a time.

Pairs are recorded with [`rigger link`](/rigger/reference/link/), which also prints them on demand.

## JSON

`week --json` is the brief as data: the lists [`next`](/rigger/reference/next/) and [`release-day`](/rigger/reference/release-day/) read, and the questions waiting.

<!-- json: week -->
| Field | Type | Meaning |
| --- | --- | --- |
| `week` | string | The week read, as `2026-W37`. |
| `monday` | string | Its Monday, as `2026-09-07`. |
| `friday` | string | Its Friday, as `2026-09-11`. |
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
| `shipping` | array | The queue for Friday: versions aimed at this week with no tag yet. |
| `shipping[]` | object | One queued version. |
| `shipping[].project` | string | The project. |
| `shipping[].version` | string | The version's number. |
| `shipping[].title` | string or null | The stage's title, or `null`. |
| `shipped` | array | The versions whose tag is already in the week. |
| `shipped[]` | object | A version whose tag is in the week. |
| `shipped[].project` | string | The project. |
| `shipped[].version` | string | The version's number. |
| `shipped[].day` | string | The day of the tag, as `YYYY-MM-DD`. |
| `shipped[].on_release_day` | boolean | Whether the tag landed on the Friday the week releases on. |
| `waiting` | array | The open questions for the owner. |
| `waiting[]` | object | One open question. |
| `waiting[].project` | string | The project that asked. |
| `waiting[].id` | integer | The question's id. |
| `waiting[].date` | string | The day it was asked, as `YYYY-MM-DD`. |
| `waiting[].body` | string | The question itself. |
| `waiting[].due` | string or null | The day an answer is needed by, as `YYYY-MM-DD`, or `null`. |
| `waiting[].overdue` | boolean | Whether that day has gone by. |
| `shared` | array | Groups where one answer settles several projects. |
| `shared[]` | object | One such group. |
| `shared[].subject` | string | What the projects share. |
| `shared[].projects` | array | The projects the answer would settle. |
| `shared[].projects[]` | string | One project. |
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

## Related

- [`link`](/rigger/reference/link/) - recording the pairs this section checks.
- [`release-day`](/rigger/reference/release-day/) - the queue in full.
- [`next`](/rigger/reference/next/) - the same week with what is past its date.
- [`inbox`](/rigger/reference/inbox/) - the questions themselves.
- [`digest`](/rigger/reference/digest/) - what moved, once the week is over.
