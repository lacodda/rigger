---
title: queue
description: What comes next - every project in the order it gets a turn, the reason for each place, and how to move one to the front.
---

```
rigger queue [--json]
rigger queue first <PROJECT> [--why <TEXT>]
rigger queue back <PROJECT>
```

Every project in the work, in the order it gets a turn. The first is what to take up now, with its stage and the reason it is first; the rest wait behind it by the same rule; the projects the rule does not consider are listed apart. `rigger` with no command prints the same thing.

```console
$ rigger queue
Next: widget  v0.2.0 · Plugins
  not finished: 1 change since the last release
  1 question waits on the owner · 1 order from neighbours

Then
  sample  v0.2.0  no turn yet
  gadget  v1.0.0  waited 3 releases

Left out
  archive  weight 0 - worked on when asked

Pairs out of step
  widget v0.2.0 shipped without gadget v0.2.0
```

[`rigger go`](/rigger/reference/go/) takes the same answer and starts your assistant on it.

## The rule

An order of reasons. The first reason that holds for a project is its reason, and a project with a stronger reason comes before every project with a weaker one:

| Reason | Holds when | Within it, first |
| --- | --- | --- |
| `unfinished` | a turn begun and not over: work not committed or not pushed, an operation git left halfway, a feature or a fix since the last release, a release that did not get out, a sitting left open | the one waiting longest |
| `moved` | something asked for it to come first, and it has not had its turn since | the one moved longest ago |
| `turn` | nothing presses | the highest `(waited + 1) * weight` |

For a `turn`, `waited` is how many releases the line has made since this project's last one, and `weight` is what [`priority`](/rigger/reference/priority/) says. A folder's turn is the end of its last sitting.

The count is in releases, not days, so nothing here is tied to a date: a fortnight off changes nothing about whose turn it is. And the formula gives each project exactly its weight's share of the turns, spaced evenly - weight 3 comes round three times as often as weight 1. A release made by hand counts as its turn.

The order is worked out from the record each time and never stored. There is no list to fall out of step: change a weight and the order has changed, ship a version and that was its turn.

**`unfinished` is what stops the line.** While a tag is not out, the next project does not start: [`go`](/rigger/reference/go/) comes back to this one, and only `--force` walks past it. It reads what is true of the repository, not what any sitting knew: a feature left untagged on purpose is still a feature nobody has. A commit that is neither a feature nor a fix - docs, a chore - waits for the next tag without holding anything up, and a stash is not counted. A release is asked after only when nothing speaks for it: one the release engine or GitHub reported is taken at its word, and a tag nothing has reported is looked for on the remote.

**A blocker is the way past something out of reach.** A project waiting on the outside world would otherwise stop every other. [`rigger note --kind blocker`](/rigger/reference/note/) takes it out of the queue with that as its reason, until [`resolve`](/rigger/reference/resolve/) clears it or a later release settles it.

**Questions do not take a project out.** A question to the owner does not stop a stage, so it does not stop the queue either; the questions waiting, and the orders [neighbours](/rigger/reference/link/) placed, are counted on the line.

## Git is read first

Before choosing, `queue` reads every repository in the work as [`sync`](/rigger/reference/sync/) does - tags and changes, without asking GitHub - so the reason stands on today's commits, not on the last sitting's. A repository that does not read is named at the end, and its facts are as of the last time it did.

## Left out

| Why | Meaning |
| --- | --- |
| `blocked` | something out of reach stops its tag: the [blocker](/rigger/reference/go/#when-something-out-of-reach-stops-the-tag) is named beside it |
| `out` | its weight is 0: worked on when asked |
| `put-down` | every open task of everything planned is frozen or asleep |

A project with nothing planned is in the queue: its turn is what plans it, as [`tail`](/rigger/reference/tail/) says. A project set aside on purpose stays out, and so does whatever it was left holding.

## Pairs out of step

After the projects, the queue prints what [`rigger link drift`](/rigger/reference/link/#drift) prints: the [pairs](/rigger/reference/link/) whose halves have parted company. A pair belongs to no single project, so no screen that reads one project at a time would ever show it.

## `queue first`

```
rigger queue first <PROJECT> [--why <TEXT>]
```

Puts a project first until its next release.

```console
$ rigger queue first gadget --why "the owner asked"
gadget comes first until its next release: the owner asked
```

It is recorded as an event of the kind `moved`, with the reason as its first line, and it stays open until the project's next turn. [`rigger resolve`](/rigger/reference/resolve/) closes it earlier; its kind then becomes `returned`. A project that was out or put down comes back into the queue when it is moved up: asking for it is enough. Only an `unfinished` project still comes before it.

## `queue back`

```
rigger queue back <PROJECT>
```

Puts it back in its turn.

```console
$ rigger queue back gadget
gadget is back in its turn.
```

## Over MCP

The same answer is the [`queue` tool](/rigger/reference/mcp/#tools), so an assistant told to get on with the line, with no project named, asks the record which one and then reads that project's [`context`](/rigger/reference/context/). `move_first` is `queue first`.

## The JSON is a contract

Every field is named here, and the test suite fails when the JSON prints one this page does not name. `queue first` and `queue back` print the queue as it now stands, in the same shape. Every item of `order` has one shape.

<!-- json: queue, queue first, queue back -->
| Field | Type | Meaning |
| --- | --- | --- |
| `order` | array | Every project that gets a turn, in the order it gets it; the first is next. |
| `order[]` | object | One project. |
| `order[].project` | string | The project. |
| `order[].kind` | string | `repo` or `folder`. |
| `order[].weight` | integer | Its [weight](/rigger/reference/priority/). |
| `order[].reason` | string | Why it comes where it does: `unfinished`, `moved` or `turn`. |
| `order[].version` | string | The version to work on: the current stage; `unplanned` for a project left unfinished with no stage. |
| `order[].title` | string or null | That version's title, or `null`. |
| `order[].unfinished` | object or null | What it holds that has not gone out, for `unfinished`; otherwise `null`. |
| `order[].unfinished.changed` | integer | Tracked files changed and not committed. |
| `order[].unfinished.untracked` | integer | Files git does not know and does not ignore. |
| `order[].unfinished.unpushed` | integer | Commits on no remote; none is counted in a repository with no remote. |
| `order[].unfinished.operation` | string or null | An operation git left halfway: `merge`, `rebase`, `cherry-pick`, `revert`, `bisect` or `am`. |
| `order[].unfinished.untagged` | array | The subjects of the features and fixes since the last release, oldest first. |
| `order[].unfinished.untagged[]` | string | One change's subject line. |
| `order[].unfinished.undelivered` | string or null | How the last release fell short of getting out, in a few words. |
| `order[].unfinished.sitting` | string or null | When the sitting left open began. |
| `order[].moved` | object or null | The request that put it first, while it stands; otherwise `null`. |
| `order[].moved.id` | integer | The id [`resolve`](/rigger/reference/resolve/) closes it by. |
| `order[].moved.head` | string | The line it is known by: the reason given. |
| `order[].moved.since` | string | The day it was recorded, as `YYYY-MM-DD`. |
| `order[].waited` | integer | Releases the line has made since this project's last one. |
| `order[].last_turn` | string or null | The day of its last sitting, as `YYYY-MM-DD`; `null` if it never had one. |
| `order[].waiting` | integer | Its questions waiting on the owner. |
| `order[].refactor` | boolean | Its next turn is a [refactoring](/rigger/reference/refactor/) instead of a stage. |
| `order[].asked` | integer | The orders its neighbours placed with it, still open. |
| `left_out` | array | The projects the rule does not consider. |
| `left_out[]` | object | One of them. |
| `left_out[].project` | string | The project. |
| `left_out[].why` | string | `blocked`, `out` or `put-down`. |
| `left_out[].blocker` | object or null | What blocks it, for `blocked`; the oldest of them. |
| `left_out[].blocker.id` | integer | The id [`resolve`](/rigger/reference/resolve/) clears it by. |
| `left_out[].blocker.head` | string | The line it is known by. |
| `left_out[].blocker.since` | string | The day it was recorded, as `YYYY-MM-DD`. |
| `unread` | array | The projects whose git did not read. |
| `unread[]` | string | One project's name. |
| `parted` | array | The pairs out of step, in the shape `link drift` prints; one row per pair. |
| `parted[]` | object | One pair out of step. |
| `parted[].drift` | string | `shipped-alone` when one half has a tag and the other has not, `run-ahead` when one half is two or more versions past the other. |
| `parted[].ahead` | object | The end that shipped or went on ahead. |
| `parted[].ahead.project` | string | The name of the project at this end. |
| `parted[].ahead.version` | string | The version anchored on this end, as the record spells it; null when none is. |
| `parted[].ahead.shipped` | boolean | Whether that version has a tag; null when no version is anchored. |
| `parted[].behind` | object | The end left behind. |
| `parted[].behind.project` | string | The name of the project at this end. |
| `parted[].behind.version` | string | The version anchored on this end, as the record spells it; null when none is. |
| `parted[].behind.shipped` | boolean | Whether that version has a tag; null when no version is anchored. |
| `parted[].versions` | integer or null | The versions between the two ends for `run-ahead`; null for `shipped-alone`. |
| `parted[].note` | string or null | The note the link was recorded with. |

Fields may be added; none is renamed or removed without a major version.

## Moved up

A project can come before its turn, and the turn it takes is the one it had coming, taken earlier: it gets no more of them for being moved. Four things move one, each settled without anyone having to remember it:

| What | Settled by |
| --- | --- |
| `rigger queue first` - the owner's word | its next release, or `queue back` |
| A release of something it [builds against](/rigger/reference/link/#what-a-project-builds-against) with a breaking change, since its own last release | its next release |
| An order of its own, [built](/rigger/reference/resolve/) and released, since its own last release | its next release |
| A neighbour's order it holds, placed with [`wish --waits`](/rigger/reference/note/): that stage cannot go on without it | resolving the order |

An ordinary release of a library moves nobody: its users take it up in their own turn. Only the owner's word brings back a project at weight 0 or put down; the others leave it where it is.

## Related


- [`priority`](/rigger/reference/priority/) - the weight that sets how often a project comes round.
- [`go`](/rigger/reference/go/) - the same answer, with your assistant started on it.
- [`resolve`](/rigger/reference/resolve/) - closes a request to come first, or a blocker.
- [`link`](/rigger/reference/link/) - the pairs whose drift the queue ends with.
- [`context`](/rigger/reference/context/) - what to read once the project is chosen.
