---
title: refactor
description: Whose turn it is to be refactored, one at a time across the line, and how it is done.
---

```
rigger refactor
rigger refactor ritual
rigger refactor done <PROJECT> <TEXT>
```

Code that is only ever added to gets slower to change, and making it easier to work on is the kind of work that never comes first by itself. So it is rationed across all the projects and taken in turn.

```console
$ rigger refactor
Next to refactor: widget, on its next turn

  widget  34 versions and none yet
  gadget  12 versions since 2026-08-14
  sample  6 versions and none yet
```

## Who goes next

The project with the most versions shipped since its last refactoring, once it has at least `refactor.min_versions` of them, while the line has done fewer than `refactor.per_week` this week. Versions rather than days, because what makes code harder to work on is the work done to it: a project that ships twenty a month comes round sooner than one that ships two.

It takes the project's own turn, not an extra one, and moves nothing in the [queue](/rigger/reference/queue/): when the project next comes up, [`rigger go`](/rigger/reference/go/) says this turn is a refactoring instead of the stage. A folder has no code to refactor, and a project at weight 0 or blocked is passed over.

## The ritual

`rigger refactor ritual` prints how it is done, and an assistant on the turn follows it. In short:

1. **Measure first:** size, what changes most, how long tests and builds take, what ships, the slowest queries.
2. **Choose by where it hurts:** what is both complicated and often changed, one to three zones forming a theme. The rest goes to `wish`.
3. **Pin down what it does today:** characterisation tests, and real output saved to compare.
4. **Small steps, one hat at a time:** no features and no bug fixes in a refactoring; one mechanical move to a commit, the gate green after each.
5. **The tests themselves**, in commits of their own.
6. **The database:** read the plans before touching an index, let the schema carry the rules, rehearse migrations on a copy of real data, expand first and contract later.
7. **Dependencies and the build.**
8. **What does not move:** commands, JSON fields, file formats, what a user already has.
9. **Measure again, and decide:** better goes in the release notes as a table of before and after; not better is reverted; nothing worth doing is a good answer.
10. **Finish:** a patch of the last release, and `refactor done`.

## `refactor done`

Says a project was refactored, or looked at and did not need it, with what was measured. That starts the count to its next one, and takes this week's slot. Over MCP it is `refactor_done`.

## The numbers

`refactor.min_versions` and `refactor.per_week` are [settings](/rigger/reference/config/); a project may have its own minimum.

## JSON

`refactor done` prints the same as `refactor`.

<!-- json: refactor, refactor done -->
| Field | Type | Meaning |
| --- | --- | --- |
| `next` | string or null | The project whose next turn is a refactoring; `null` when the week's slot is taken or nobody has gone long enough. |
| `this_week` | array | The refactorings done this week. |
| `this_week[]` | object | One of them. |
| `this_week[].project` | string | The project. |
| `this_week[].at` | string | When. |
| `per_week` | integer | How many a week are allowed. |
| `standings` | array | Every project it applies to, the longest gone first. |
| `standings[]` | object | One of them. |
| `standings[].project` | string | The project. |
| `standings[].versions` | integer | Versions shipped since its last refactoring, or ever. |
| `standings[].min_versions` | integer | How many it takes to be considered. |
| `standings[].last` | string or null | When it was last done. |

<!-- json: refactor ritual -->
| Field | Type | Meaning |
| --- | --- | --- |
| `ritual` | string | The text the plain command prints. |

## Related

- [`queue`](/rigger/reference/queue/) - the turn it takes.
- [`tail`](/rigger/reference/tail/) - the other thing a turn can start with.
- [`config`](/rigger/reference/config/) - the numbers.
