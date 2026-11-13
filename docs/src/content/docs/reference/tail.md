---
title: tail
description: How much of each plan is left, and whether a turn should start by growing or sorting it.
---

```
rigger tail [PROJECT]
rigger tail ritual
rigger tail done <PROJECT> <TEXT>
```

A plan nobody adds to is a product that stops the day it runs out. So the record watches how much is left of each plan, and when something should be done about it, a project's next turn starts by doing that.

```console
$ rigger tail
widget   2 versions planned, at or under 3 - grow it
gadget   12 versions planned - sort what has piled up (9 wishes)
sample   44 versions planned, at the cap of 30: small ideas wait
```

Name a project, `rigger tail widget`, for its line alone - the way every other command takes one; until v0.50.0 it took `--project`, which the ritual's own text never used.

rigger has no ideas of its own. It says when; [`rigger tail ritual`](#the-ritual) says how; the assistant on the turn does the thinking. [`rigger go`](/rigger/reference/go/) puts it in the first message, and the [packet](/rigger/reference/context/) says it near the top.

## What can be due

| Due | When | What the turn starts with |
| --- | --- | --- |
| `grow` | `tail.floor` versions planned or fewer | Sorting what piled up, then new versions: small ideas straight into the plan, big ones raised with [`rigger ideas add`](/rigger/reference/ideas/) |
| `sort` | `tail.sort_days` days since the plan was last gone through, or since the project was added | Only sorting the wishes and orders waiting |

The stage still goes to a release in the same sitting. A project with nothing planned at all stays in the [queue](/rigger/reference/queue/): its turn is what plans it. A folder has no such plan, and a project at weight 0 is not asked about.

`tail.cap` is not a limit on anything rigger does. It is where the ritual stops adding small ideas by themselves, and the screen says so.

## `tail done`

Says the plan has been gone through, with what changed, so this turn does not ask again. It asks once a turn at most: an assistant that looked and honestly found nothing good to add is not sent to look again until something has shipped. Over MCP it is `tail_done`.

## The ritual

`rigger tail ritual` prints how a plan is grown and sorted: sort what is already asked for before inventing anything, where to look for ideas, and what makes an idea small enough to go straight in. Every idea needs a source to name - an order, a finding, a measurement, the owner's words - so that the plan does not fill up with ideas for the sake of the plan.

## The numbers

`tail.floor`, `tail.cap` and `tail.sort_days` are [settings](/rigger/reference/config/), for the whole record or for one project.

## JSON

<!-- json: tail -->
| Field | Type | Meaning |
| --- | --- | --- |
| `tails` | array | One per project in the work. |
| `tails[]` | object | One project's tail. |
| `tails[].project` | string | The project. |
| `tails[].planned` | integer | Versions planned and not shipped. |
| `tails[].floor` | integer | Its `tail.floor`. |
| `tails[].cap` | integer | Its `tail.cap`. |
| `tails[].unsorted` | integer | Wishes and orders waiting to be sorted. |
| `tails[].last_pass` | string or null | When the plan was last gone through. |
| `tails[].days_since` | integer or null | Days since then, or since the project was added. |
| `tails[].due` | string or null | `grow`, `sort`, or `null` when nothing is due. |

`tail done` prints the project's tail as it now stands:

<!-- json: tail done -->
| Field | Type | Meaning |
| --- | --- | --- |
| `project` | string | The project. |
| `planned` | integer | Versions planned and not shipped. |
| `floor` | integer | Its `tail.floor`. |
| `cap` | integer | Its `tail.cap`. |
| `unsorted` | integer | Wishes and orders waiting to be sorted. |
| `last_pass` | string or null | When the plan was last gone through. |
| `days_since` | integer or null | Days since then. |
| `due` | string or null | `grow`, `sort`, or `null`. |

<!-- json: tail ritual -->
| Field | Type | Meaning |
| --- | --- | --- |
| `ritual` | string | The text the plain command prints. |

## Related

- [`ideas`](/rigger/reference/ideas/) - where big ideas wait.
- [`config`](/rigger/reference/config/) - the numbers.
- [`go`](/rigger/reference/go/) - the turn this starts.
