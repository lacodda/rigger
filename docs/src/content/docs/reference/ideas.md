---
title: ideas
description: Big ideas waiting for your answer, asked about together rather than one at a time.
---

```
rigger ideas
rigger ideas add <PROJECT> <TEXT>
rigger ideas take <PROJECT> <ID> [--note <TEXT>]
rigger ideas drop <PROJECT> <ID> [--note <TEXT>]
rigger ideas asked
```

Growing a plan turns up two kinds of idea. A small one [goes straight in](/rigger/reference/tail/#the-ritual). A big one - a new surface, a companion program, a change to the model of the data or to what the product is - waits here for your answer.

```console
$ rigger ideas
2 big ideas wait
  widget [41] 2026-11-02  A companion program for the phone
  gadget [57] 2026-11-05  Sync through a relay instead of files

Time to ask: have your assistant put them in one questionnaire, then `rigger ideas take` or `drop` each, and `rigger ideas asked`.
```

They are kept apart from the [inbox](/rigger/reference/inbox/). A question there is something a stage is held up by, or will be; an idea holds nothing up, and a list with both in it serves neither. They are asked about together instead, across every project, once every `ideas.every_days` days: thinking about one product's future goes better beside the others', and a single questionnaire gets answered where a long list of separate questions does not.

An idea is not work yet: it is not in the project's [packet](/rigger/reference/context/), and nothing is built on it before it is answered.

## When it is time

When there are ideas and it has been `ideas.every_days` days, [`rigger queue`](/rigger/reference/queue/) says so at its foot, and this page's command says what to do. Any assistant can make the questionnaire from `rigger ideas --json`. `rigger ideas asked` starts the count again, once you have been asked.

## Answering

`take` makes the idea a wish of its project, so the next [sort](/rigger/reference/tail/) gives it a place in the plan. `drop` puts it away: it stays in the record, findable, and leaves the list. A `--note` goes with either as a decision, as an answer to a question does.

Over MCP, `raise_idea` raises one. Answering is yours.

## JSON

Every form prints the list as it now stands.

<!-- json: ideas, ideas add, ideas take, ideas drop, ideas asked -->
| Field | Type | Meaning |
| --- | --- | --- |
| `ideas` | array | The ideas waiting, oldest first. |
| `ideas[]` | object | One idea. |
| `ideas[].id` | integer | Its id. |
| `ideas[].project` | string | Its project. |
| `ideas[].body` | string | The idea, whole. |
| `ideas[].date` | string | The day it was raised, `YYYY-MM-DD`. |
| `due` | boolean | Whether it is time to ask. |
| `asked_at` | string or null | When the owner was last asked. |

## Related

- [`tail`](/rigger/reference/tail/) - how ideas come up.
- [`inbox`](/rigger/reference/inbox/) - the questions that hold a stage up.
