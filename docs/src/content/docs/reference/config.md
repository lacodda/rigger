---
title: config
description: The numbers you can set - how plans grow, and when.
---

```
rigger config [KEY]
rigger config <KEY> <VALUE> [--project <PROJECT>]
rigger config <KEY> --clear [--project <PROJECT>]
```

The few numbers that say how the line is kept up, each with the default it was run on. Set one for the whole record, or for one project whose needs are not the others'.

```console
$ rigger config
tail.floor       3  With this many versions planned or fewer, a project's turn starts by growing its plan
tail.cap         30  With this many versions planned, small ideas stop going into the plan by themselves
tail.sort_days   30  How many days a project's wishes and orders may wait before a turn starts by sorting them
ideas.every_days 30  How many days between the questionnaires that ask about big ideas

$ rigger config tail.floor 5 --project widget
```

A project's own value comes first, then the record's, then the default. `--clear` takes a value away, back to what stood before it. Values are whole numbers and none is below 0; `tail.sort_days 0` turns sorting by the calendar off. A change counts from the next look at the queue.

## JSON

<!-- json: config -->
| Field | Type | Meaning |
| --- | --- | --- |
| `settings` | array | The settings; just the one when a key is named. |
| `settings[]` | object | One setting. |
| `settings[].key` | string | Its name. |
| `settings[].value` | integer | The record's value. |
| `settings[].default` | integer | The value with nothing set. |
| `settings[].about` | string | What it means. |
| `settings[].projects` | array | The projects with a value of their own. |
| `settings[].projects[]` | object | One of them. |
| `settings[].projects[].project` | string | The project. |
| `settings[].projects[].value` | integer | Its value. |

## Related

- [`tail`](/rigger/reference/tail/) and [`ideas`](/rigger/reference/ideas/) - what the numbers steer.
