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
  not finished: 1 change since the last release
  1 question waits on the owner · 1 order from neighbours

Then, by the same rule
  gadget  v1.0.0  last sitting 2026-10-27
  sample  v0.2.0  last sitting 2026-10-27

Left out
  archive  out of the rotation - worked on when asked
```

The [calendar](/rigger/reference/calendar/) answers the same question when a version has been aimed at the week. Most weeks of a busy line have nothing aimed at them, and `pick` answers anyway. [`rigger go`](/rigger/reference/go/) takes the same answer and starts your assistant on it.

## The rule

An order of reasons, not a sum of scores. The first reason that holds for a project is its reason, and a project with a stronger reason comes before every project with a weaker one:

| Reason | Holds when | Within it, first |
| --- | --- | --- |
| `unfinished` | a turn begun and not over: work not committed or not pushed, an operation git left halfway, a feature or a fix since the last release, a release that did not get out, a sitting left open | the one waiting longest |
| `late` | a version is aimed at a week already gone, with no tag | the longest past its week, then the higher tier |
| `due` | a version is aimed at this week | the higher tier, then the longest without a sitting |
| `answered` | the owner decided something about it outside a sitting, after its last one | the one waiting longest |
| `behind` | its [tier](/rigger/reference/project/#project-tier) asks for more - a floor broken, or a rhythm lapsed | a floor before a rhythm, then the higher tier, then the longer lapse |
| `turn` | nothing presses | the longest without a sitting; never having had one is longest of all |

A reason is a sentence you can check, and steer: aim a version at the week with [`version plan`](/rigger/reference/version/) and it comes first; give a project a tier and its rhythm is kept. There are no weights to tune.

**`unfinished` is what stops the line.** While a tag is not out, the next project does not start: [`go`](/rigger/reference/go/) comes back to this one, and only `--force` walks past it. It reads what is true of the repository, not what any sitting knew: a feature left untagged on purpose is still a feature nobody has. A commit that is neither a feature nor a fix - docs, a chore - waits for the next tag without holding anything up, and a stash is not counted: nobody means to finish one by the next tag. A release is asked after only when nothing speaks for it: one the release engine or GitHub reported is taken at its word, and a tag nothing has reported is looked for on the remote.

**A blocker is the way past something out of reach.** A project waiting on the outside world would otherwise stop every other. [`rigger note --kind blocker`](/rigger/reference/note/) takes it out of the order with that as its reason, until [`resolve`](/rigger/reference/resolve/) clears it or a later release settles it.

**Questions do not take a project out.** A question to the owner does not stop a stage, so it does not stop the pick either; the questions waiting, and the orders [neighbours](/rigger/reference/link/) placed, are counted on the line.

**The tier C signal is not a reason.** It says a second declared product started before the first shipped - which is a reason to work on the first one, not the one it names.

## Git is read first

Before choosing, `pick` reads every repository in the work as [`sync`](/rigger/reference/sync/) does - tags and changes, without asking GitHub - so the reason stands on today's commits, not on the last sitting's. A commit made in one project from another project's sitting is exactly what the record would otherwise not know. A repository that does not read is named at the end, and its facts are as of the last time it did.

## Left out

| Why | Meaning |
| --- | --- |
| `blocked` | something out of reach stops its tag: the [blocker](/rigger/reference/go/#when-something-out-of-reach-stops-the-tag) is named beside it |
| `out` | its tier is `out`: worked on when asked |
| `no-stage` | nothing is planned for it, so there is no stage to work on |
| `put-down` | every open task of everything planned is frozen or asleep |

A version aimed at a week brings a project back in whatever else is true of it, a blocker aside: aiming a version is asking for it. A project left unfinished is in the order even with no stage planned - work done with nothing planned still has to go out - and out of it when it was set aside on purpose. A place the record keeps for itself (`project service`) is not work and is not listed at all.

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
| `pick.reason` | string | Why it comes where it does: `unfinished`, `late`, `due`, `answered`, `behind` or `turn`. |
| `pick.version` | string | The version to work on: the one aimed at a week for `late` and `due`, the current stage otherwise; `unplanned` for a project left unfinished with no stage. |
| `pick.title` | string or null | That version's title, or `null`. |
| `pick.planned` | string or null | The week the version is aimed at, for `late` and `due`; otherwise `null`. |
| `pick.weeks_late` | integer or null | How many weeks that week is past, for `late`; otherwise `null`. |
| `pick.unfinished` | object or null | What it holds that has not gone out, for `unfinished`; otherwise `null`. |
| `pick.unfinished.changed` | integer | Tracked files changed and not committed. |
| `pick.unfinished.untracked` | integer | Files git does not know and does not ignore. |
| `pick.unfinished.unpushed` | integer | Commits on no remote; none is counted in a repository with no remote. |
| `pick.unfinished.operation` | string or null | An operation git left halfway: `merge`, `rebase`, `cherry-pick`, `revert`, `bisect` or `am`. |
| `pick.unfinished.untagged` | array | The subjects of the features and fixes since the last release, oldest first. |
| `pick.unfinished.untagged[]` | string | One change's subject line. |
| `pick.unfinished.undelivered` | string or null | How the last release fell short of getting out, in a few words. |
| `pick.unfinished.sitting` | string or null | When the sitting left open began. |
| `pick.answered` | integer | How many things the owner decided outside a sitting since its last one. |
| `pick.behind` | object or null | How its tier asks for more; `null` when it does not. |
| `pick.behind.lag` | string | `missed-cycle` (tier A), `without-focus` (tier B) or `rhythm` (longer without a release than its rhythm). |
| `pick.behind.weeks` | integer | Weeks without a release - without a turn, for `without-focus`. |
| `pick.behind.rhythm_weeks` | integer or null | Weeks between releases its tier asks for. |
| `pick.last_turn` | string or null | The day of its last sitting, as `YYYY-MM-DD`; `null` if it never had one. |
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
| `then[].unfinished` | object or null | What it holds that has not gone out, or `null`. |
| `then[].unfinished.changed` | integer | Tracked files changed and not committed. |
| `then[].unfinished.untracked` | integer | Files git does not know and does not ignore. |
| `then[].unfinished.unpushed` | integer | Commits on no remote; none is counted in a repository with no remote. |
| `then[].unfinished.operation` | string or null | An operation git left halfway: `merge`, `rebase`, `cherry-pick`, `revert`, `bisect` or `am`. |
| `then[].unfinished.untagged` | array | The subjects of the features and fixes since the last release, oldest first. |
| `then[].unfinished.untagged[]` | string | One change's subject line. |
| `then[].unfinished.undelivered` | string or null | How the last release fell short of getting out, in a few words. |
| `then[].unfinished.sitting` | string or null | When the sitting left open began. |
| `then[].answered` | integer | Owner's decisions since its last sitting. |
| `then[].behind` | object or null | How its tier asks for more, or `null`. |
| `then[].behind.lag` | string | `missed-cycle`, `without-focus` or `rhythm`. |
| `then[].behind.weeks` | integer | Weeks behind. |
| `then[].behind.rhythm_weeks` | integer or null | Its rhythm in weeks. |
| `then[].last_turn` | string or null | The day of its last sitting, or `null`. |
| `then[].waiting` | integer | Questions waiting on the owner. |
| `then[].asked` | integer | Open orders from neighbours. |
| `left_out` | array | The projects the rule does not consider. |
| `left_out[]` | object | One of them. |
| `left_out[].project` | string | The project. |
| `left_out[].why` | string | `blocked`, `out`, `no-stage` or `put-down`. |
| `left_out[].blocker` | object or null | What blocks it, for `blocked`; the oldest of them. |
| `left_out[].blocker.id` | integer | The id [`resolve`](/rigger/reference/resolve/) clears it by. |
| `left_out[].blocker.head` | string | The line it is known by. |
| `left_out[].blocker.since` | string | The day it was recorded, as `YYYY-MM-DD`. |
| `unread` | array | The projects whose git did not read. |
| `unread[]` | string | One project's name. |

Fields may be added; none is renamed or removed without a major version.

## Related

- [`week`](/rigger/reference/week/) - the Monday brief, which names the pick when nothing is aimed.
- [`next`](/rigger/reference/next/) - the calendar's own focus, and the tier signals `behind` reads.
- [`version plan`](/rigger/reference/version/) - aim a version at a week, and it comes first.
- [`project tier`](/rigger/reference/project/) - the rhythm `behind` keeps.
- [`context`](/rigger/reference/context/) - what to read once the project is picked.
