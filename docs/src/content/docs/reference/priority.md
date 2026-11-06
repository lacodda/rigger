---
title: priority
description: How often each project comes round - a weight, shown or changed.
---

```
rigger priority [PROJECT] [up|down|N] [--json]
```

A weight says how often a project gets a turn in the [queue](/rigger/reference/queue/). Weight 3 comes round three times as often as weight 1. Weight 0 leaves the queue: the project is worked on when asked. A new project has weight 2.

```console
$ rigger priority
widget   3  3 turns in every 6
gadget   2  2 turns in every 6
sample   1  1 turn in every 6
archive  0  out of the queue
```

Heaviest first. The second figure reads the weight against the others: out of so many turns of the line, this many are the project's.

## One project

```console
$ rigger priority widget
widget has weight 3
```

`up` and `down` move it one step, and a whole number sets it:

```console
$ rigger priority widget up
widget has weight 4
$ rigger priority archive 1
archive has weight 1
```

A weight of 0 takes the project out:

```console
$ rigger priority gadget 0
gadget has weight 0: out of the queue, worked on when asked
```

A project that is out is not forgotten: [`rigger go gadget`](/rigger/reference/go/#naming-a-project) works on it, and [`queue first`](/rigger/reference/queue/#queue-first) brings it back for a turn.

## Over MCP

The tool is `set_priority`, with a `project` and a `change`: `up`, `down` or a whole number.

## JSON

Without a project, the weights of all; with one, that project's.

<!-- json: priority -->
| Field | Type | Meaning |
| --- | --- | --- |
| `weights` | array | Every project's weight, heaviest first; present without a project. |
| `weights[]` | object | One project. |
| `weights[].project` | string | The project. |
| `weights[].weight` | integer | Its weight. |
| `project` | string | The project asked about; present with a project. |
| `weight` | integer | Its weight; present with a project. |

## Related

- [`queue`](/rigger/reference/queue/) - the order the weights make.
- [`project`](/rigger/reference/project/) - where a project is added.
