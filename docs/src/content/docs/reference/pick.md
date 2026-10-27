---
title: pick
description: What to work on now - one project and its stage, the reason it comes first, and the order the rest wait in.
---

```
rigger pick [--json]
```

One name, across every project in the work: which one to take up now, the stage it is on, and the reason it comes before the others. Then the order the rest wait in, by the same rule, and the projects the rule does not consider.

```console
$ rigger pick
Work on widget  v0.2.0 · Plugins
  1 change since the last release that no finished sitting has seen: feat(plugin): load plugins from a folder
  1 question waits on the owner · 1 order from neighbours

Then, by the same rule
  gadget  v1.0.0  last sitting 2026-10-27
  sample  v0.2.0  last sitting 2026-10-27

Left out
  archive  out of the rotation - worked on when asked
```

The [calendar](/rigger/reference/calendar/) answers the same question when a version has been aimed at the week. Most weeks of a busy line have nothing aimed at them, and `pick` answers anyway.

## The rule

An order of reasons, not a sum of scores. The first reason that holds for a project is its reason, and a project with a stronger reason comes before every project with a weaker one:

| Reason | Holds when | Within it, first |
| --- | --- | --- |
| `late` | a version is aimed at a week already gone, with no tag | the longest past its week, then the higher tier |
| `due` | a version is aimed at this week | the higher tier, then the longest without a sitting |
| `unseen` | a change landed since the last release that no finished sitting of the project has seen | the one waiting longest |
| `answered` | the owner decided something about it outside a sitting, after its last one | the one waiting longest |
| `behind` | its [tier](/rigger/reference/project/#project-tier) asks for more - a floor broken, or a rhythm lapsed | a floor before a rhythm, then the higher tier, then the longer lapse |
| `turn` | nothing presses | the longest without a sitting; never having had one is longest of all |

A reason is a sentence you can check, and steer: aim a version at the week with [`version plan`](/rigger/reference/version/) and it comes first; give a project a tier and its rhythm is kept. There are no weights to tune.

**`unseen` is narrower than "commits since the tag".** A change made from a neighbour's sitting, or in a sitting that was never closed, is work the record has lost the thread of. A change a sitting made and chose not to tag is not: the sitting knew. Counting it would pick the same project on every run while it waits on something outside it. It is counted on the project's line instead - "1 change waits for the next tag".

**Questions do not take a project out.** A question to the owner does not stop a stage, so it does not stop the pick either; the questions waiting, and the orders [neighbours](/rigger/reference/link/) placed, are counted on the line.

**The tier C signal is not a reason.** It says a second declared product started before the first shipped - which is a reason to work on the first one, not the one it names.

## Git is read first

Before choosing, `pick` reads every repository in the work as [`sync`](/rigger/reference/sync/) does - tags and changes, without asking GitHub - so the reason stands on today's commits, not on the last sitting's. A commit made in one project from another project's sitting is exactly what the record would otherwise not know. A repository that does not read is named at the end, and its facts are as of the last time it did.

## Left out

| Why | Meaning |
| --- | --- |
| `out` | its tier is `out`: worked on when asked |
| `no-stage` | nothing is planned for it, so there is no stage to work on |
| `put-down` | every open task of everything planned is frozen or asleep |

A version aimed at a week brings a project back in whatever else is true of it: aiming a version is asking for it. A place the record keeps for itself (`project service`) is not work and is not listed at all.

## Over MCP

The same answer is the [`pick` tool](/rigger/reference/mcp/#tools), so an assistant told to get on with the line, with no project named, asks the record which one and then reads that project's [`context`](/rigger/reference/context/). The [Monday brief](/rigger/reference/week/#focus) names the pick when nothing is aimed at the week.

## The JSON is a contract

Every field is named here, and the test suite fails when the JSON prints one this page does not name. `pick` and every item of `then` have one shape.

<!-- json: pick -->
| Field | Type | Meaning |
| --- | --- | --- |
| `week` | string | The current week, as `2026-W44`. |
| `pick` | object or null | The project to work on; `null` when every project is left out. |
| `pick.project` | string | The project. |
| `pick.tier` | string or null | Its tier: `A`, `B`, `C` or `out`; `null` when it has none. |
| `pick.reason` | string | Why it comes where it does: `late`, `due`, `unseen`, `answered`, `behind` or `turn`. |
| `pick.version` | string | The version to work on: the one aimed at a week for `late` and `due`, the current stage otherwise. |
| `pick.title` | string or null | That version's title, or `null`. |
| `pick.planned` | string or null | The week the version is aimed at, for `late` and `due`; otherwise `null`. |
| `pick.weeks_late` | integer or null | How many weeks that week is past, for `late`; otherwise `null`. |
| `pick.unseen` | array | The subjects of the changes since the last release that no finished sitting has seen, oldest first. |
| `pick.unseen[]` | string | One change's subject line. |
| `pick.untagged` | integer | How many changes landed since the last release, seen or not. |
| `pick.answered` | integer | How many things the owner decided outside a sitting since its last one. |
| `pick.behind` | object or null | How its tier asks for more; `null` when it does not. |
| `pick.behind.lag` | string | `missed-cycle` (tier A), `without-focus` (tier B) or `rhythm` (longer without a release than its rhythm). |
| `pick.behind.weeks` | integer | Weeks without a release - without a turn, for `without-focus`. |
| `pick.behind.rhythm_weeks` | integer or null | Weeks between releases its tier asks for. |
| `pick.last_turn` | string or null | The day of its last sitting, as `YYYY-MM-DD`; `null` if it never had one. |
| `pick.sitting_open` | boolean | Whether a sitting on it is open now. |
| `pick.waiting` | integer | Its questions waiting on the owner. |
| `pick.asked` | integer | The orders its neighbours placed with it, still open. |
| `then` | array | The rest of the order, by the same rule. |
| `then[]` | object | One project, in the shape of `pick`. |
| `then[].project` | string | The project. |
| `then[].tier` | string or null | Its tier, or `null`. |
| `then[].reason` | string | Its reason, as for `pick`. |
| `then[].version` | string | The version to work on. |
| `then[].title` | string or null | Its title, or `null`. |
| `then[].planned` | string or null | The week it is aimed at, or `null`. |
| `then[].weeks_late` | integer or null | Weeks past that week, or `null`. |
| `then[].unseen` | array | Changes no finished sitting has seen. |
| `then[].unseen[]` | string | One change's subject line. |
| `then[].untagged` | integer | Changes since the last release. |
| `then[].answered` | integer | Owner's decisions since its last sitting. |
| `then[].behind` | object or null | How its tier asks for more, or `null`. |
| `then[].behind.lag` | string | `missed-cycle`, `without-focus` or `rhythm`. |
| `then[].behind.weeks` | integer | Weeks behind. |
| `then[].behind.rhythm_weeks` | integer or null | Its rhythm in weeks. |
| `then[].last_turn` | string or null | The day of its last sitting, or `null`. |
| `then[].sitting_open` | boolean | Whether a sitting on it is open. |
| `then[].waiting` | integer | Questions waiting on the owner. |
| `then[].asked` | integer | Open orders from neighbours. |
| `left_out` | array | The projects the rule does not consider. |
| `left_out[]` | object | One of them. |
| `left_out[].project` | string | The project. |
| `left_out[].why` | string | `out`, `no-stage` or `put-down`. |
| `unread` | array | The projects whose git did not read. |
| `unread[]` | string | One project's name. |

Fields may be added; none is renamed or removed without a major version.

## Related

- [`week`](/rigger/reference/week/) - the Monday brief, which names the pick when nothing is aimed.
- [`next`](/rigger/reference/next/) - the calendar's own focus, and the tier signals `behind` reads.
- [`version plan`](/rigger/reference/version/) - aim a version at a week, and it comes first.
- [`project tier`](/rigger/reference/project/) - the rhythm `behind` keeps.
- [`context`](/rigger/reference/context/) - what to read once the project is picked.
