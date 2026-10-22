---
title: import
description: Read a notes hub into versions, tasks and events.
---

```
rigger import <PROJECT> --hub <DIR> [--check] [--json]
```

Reads a project's notes hub - the plan, the changelog and the decision log - and records what it finds. This is how a project that has been run by hand for months arrives in the database with its history intact.

```console
$ rigger import sample --hub C:\dev\sample\hub
sample:
  versions   38 added, 0 updated
  tasks      111 added, 0 updated
  decisions  9 added
  questions  2 added
```

## What it reads

| File | What comes out of it |
| --- | --- |
| `План.md` | open stages and their tasks; the questions listed under "Ждёт решения владельца" |
| `Изменения.md` | stages that shipped, with the date in their heading |
| `README.md` | the dated lines of its "Состояние" block, and the prose around them |
| `Дневник.md` | one sitting per entry |
| `Решения.md` | one decision per dated heading, body and all; the prose above the first of them becomes the `decisions` [document](/rigger/reference/doc/) |
| `Видение.md` | the `vision` document |
| `Ритуалы.md` | the `rituals` document |
| `Исследования/**/*.md` | one `research` document each |
| any other `.md`, in any folder | one `other` document each, at the path it was read from |

A stage is any heading whose first word is a version - `v0.4.0` - at whatever heading level, so both `## v0.4.0 · Title` and `### v0.4.0 · Title` are found. The title is what follows the version up to the em dash; the tail after it is bookkeeping and is not part of the title. A date anywhere in that tail marks the stage as shipped, written `2026-09-03` or `03.09.2026`, after any of the words the hubs use.

A checkbox line under a stage is one of its tasks, until the next heading at the same level or above - so a backlog list that follows a stage does not become part of it.

### The handwritten texts

The first files are parsed into versions, tasks, sittings and events. The rest are not parsed at all: they are taken in whole, as [documents](/rigger/reference/doc/), because they are the files a hub kept that the record could not rebuild - and the reason a hub had to exist at all.

Every one of them, not only the ones with names the reader knows. Until v0.31.0 a text nobody had named a kind for - a mock-up of a screen, a list of stands, the format a story is written in - was left in the hub, and the record could not promise that deleting the hub would lose nothing. Now each is an `other` document, addressed by its path in whatever script the path is written (`Макеты/Экран.md` is `макеты-экран`), titled by its first heading or else by its filename, and found again by that path on the next import. A file that carries the generated mark is a view of the record wherever it lies, and is not taken in; nor is anything whose name starts with a dot, which is a tool's and not yours.

```console
$ rigger import rigger --hub ~/vault/Projects/rigger
rigger:
  versions   2 added, 11 updated
  documents  4 added, 0 updated
```

A research note is addressed by the date its filename opens with. Two notes from one day get a number after it, so neither is lost: the titles after the date are usually not typeable as an address.

A text written through [`doc add`](/rigger/reference/doc/) or an assistant's tool has no file until an export gives it one, and the export remembers which; the same words found in the hub are that text, whether the export put them there or somebody did by hand, and not a second one. A text whose words did not change keeps the title it was given, even when its first heading says something else.

Re-reading an unchanged hub changes nothing, so a document edited with `doc edit` and not yet exported is not silently replaced by the older file it came from. A file that really changed is taken in and reported as updated. Either way the record notes the file as one it has accounted for, so an edit to it afterwards is [told apart](/rigger/reference/doctor/#behind-is-not-edited) from a file nobody touched.

### The wishes file

`Хотелки.md` is read into wishes, once, so that the file can stop being a place the record has to be told about. From then on a wish arrives through [`rigger wish`](/rigger/reference/note/) or the assistant's tool, and [`doctor`](/rigger/reference/doctor/) says so if the file fills up again.

A wish there is not a line the way a task is: the ones actually written run to several paragraphs - what it is, where, and how to fix it. So what opens a wish is a **bold dated line**, and everything up to the next one belongs to it:

```markdown
**03.09.2026 · The mark on the web is not built by the size rule**

The rule: S ≤27px, M 28–63px, L ≥64px. Two places:

- `favicon.ico` - flat fill at 32 and 48
- `icon-32.png` - flat fill

**How to fix:** `export-assets.mjs` is the model.
```

Bold alone would not do: the same wish ends in a bold line of its own, and splitting on every one of them tore one wish into six.

The template is not a wish, and neither is what is left when the file is emptied - the placeholder, or a line saying where the wishes went (`*Разобрано 11.09.2026: ...*`). Taking those in would put settled things back into a packet to be sorted a second time.

## A questionnaire the owner answered

```
rigger import <project> --answers <FILE> [--check] [--json]
```

Before a plan is reworked the owner is handed a page: a few forks with the options laid out, and a list of ideas to tick. The answers steer the next months of a project - and then the page is the only place any of it is written down. The plan that comes out says what was decided; it does not say what it was decided **against**, and that is the half that answers "why this and not that" a year later.

```json
{
  "questionnaire": "plan review 03ab557f",
  "answered": "2026-09-11",
  "forks": [{
    "question": "Where knowledge of the code comes from",
    "chosen": "nooma as a library",
    "against": ["a tree-sitter index of its own", "asking an assistant each time"],
    "why": "One index for the line, not one per product."
  }],
  "ideas": [
    { "idea": "A registry of the line as public JSON", "taken": true },
    { "idea": "A second binary for the alias", "taken": false, "why": "An alias is a link." }
  ]
}
```

| In the file | In the record | Why |
| --- | --- | --- |
| a fork | a **decision**, naming the option taken and the ones it beat | a decision that records only the winner cannot be revisited |
| an idea with `"taken": true` | a **wish** | work the owner took but has not placed yet; it gets sorted into the plan like any other |
| an idea with `"taken": false` | a **decision** - "not taking this" | an idea nobody records is an idea that comes back every quarter |

Only `idea` and, for a fork, `question` and `chosen` are required. `taken` defaults to **false**, which is the safe way round: a tick the page forgot to write becomes a visible decision not to take it, rather than a wish nobody asked for.

Events are dated by `answered`, so the reasoning sits among the other events of that day rather than at the moment of the import. Reading the same page twice records nothing twice - which is how a correction to it is applied.

`--check` says what would be taken and writes nothing. `--answers` and `--hub` are two sources, not two halves of one, so asking for both is refused.

## Checking before importing

`--check` runs the whole import and takes all of it back: the same report, and nothing in the record changed - not the plan, not the events, not the hub path the project remembers.

That path is recorded by an import only when the project has none yet. Reading a directory is not moving the hub there: before v0.31.0 an import from a scratch copy, or from a folder holding one file to take in, made that folder the project's hub until the next export. [`export`](/rigger/reference/export/) is what moves a hub, because it writes one.

```console
$ rigger import sample --check --hub C:\dev\sample\hub
sample - what an import would change; nothing was written:
  versions   3 added, 0 updated
  tasks      1 added, 0 updated
```

The same code path as the import itself, rolled back, rather than a second reading that describes the first: a check that guessed could say one thing while the import did another. Until v0.27.0 `--check` reached only `--answers`, and a hub import asked only to describe itself wrote everything.

## Running it twice

Importing an unchanged hub again changes nothing and says so:

```console
$ rigger import sample --hub C:\dev\sample\hub
sample: nothing changed
```

A stage is identified by its version, a task by its text within its stage, a decision by its date and body, and a question by its text. So a stage that moves from the plan to the changelog when it ships is updated, not duplicated.

## What the hub struck

A hub **kept by hand** is the owner's pen, and a line struck from it is struck. A hub rigger wrote is not: it carries a mark saying so, it lists the open stages and nothing else, and reading it back describes the record rather than editing it. Neither its tasks nor its versions are struck by what it no longer lists. A task the plan no longer lists is marked `dropped`: the export does not write it, the packet does not count it, and it comes back as open the moment the plan lists it again. A planned stage that neither file names any more - renumbered when the queue moved, or given up - is dropped the same way, with its open tasks. Done tasks and shipped stages stay: they are history, and a tag outranks a plan that has not been told.

```console
$ rigger import sample --hub C:\dev\sample\hub
sample:
  tasks      2 added, 1 updated
  dropped    1 version and 4 tasks struck from the hub
```

Nothing is deleted. A dropped row keeps its id and its history, and reads as what it is.

## What a stale plan cannot do

A plan that has not been told a stage shipped is behind, not a decision. Its empty boxes do not reopen the tasks the record closed, and its depth and place do not follow the stage into the changelog. Its copy of a shipped stage - a line in the major map - neither un-ships it nor writes over the changelog's entry.

## The diary

`Дневник.md` is read into sittings: one entry per heading, with the heading exactly as written. Entries are matched to the sittings the record holds a day at a time, one to one, closest first: the same heading over the same text, then the same text under a heading edited in the file, then the same heading over edited text, then a heading an older reader spelt without its date. A day may hold any number of entries - two under the same heading included.

Nothing an import reads deletes a sitting. Before v0.31.0 the second of two sittings headed by the same day found the first one's row, and a branch that merged "the same entry spelt twice" deleted the second: a release sitting vanished on an ordinary import. A sitting the file does not name stays, as everything else in the record does.

The order of the diary and the blank lines and rules between its entries come from the file only when the file is the whole diary - when it names every sitting the record keeps an entry for. A file that names some of them speaks for what it says about those: an edited heading or body is taken in, and the rest of the diary keeps its order. Read as the whole diary, a file an older `--diary` had appended to, or a folder handed one entry to take in, moved every sitting it did not name above every one it did.

## A file the record wrote

A generated file still exactly what the record last wrote into it is not read back, and the import says so:

```console
$ rigger import sample --hub C:\dev\sample\hub
note: not read back, still what the record wrote: План.md, Изменения.md, README.md
sample: nothing changed
```

Such a file is the record as it stood at that export. Reading it back could only take the record there again - wiping a state line added since, or putting back the shape of a queue the record has answered. A file somebody has written in is read as before, and that is what an import of a generated hub is for: taking in an edit.

## When a file is missing

A hub without one of its files is not an error - the others are still read, and the missing ones are named:

```console
$ rigger import sample --hub C:\dev\sample
note: План.md is missing from C:\dev\sample
note: Изменения.md is missing from C:\dev\sample
sample: nothing changed
```

And a missing file says nothing. A hub without its plan names no stage because it has no file to name them in, so nothing is struck and no question is withdrawn; one without its README leaves the state lines as they are. Until v0.31.0 a missing file read as an empty one, and a folder holding a diary alone, imported to take in that diary, struck every planned stage of the project.

## When the hub itself is missing

A directory that is not there, or that holds nothing a hub is read from, is **refused**:

```console
$ rigger import sample --hub C:\dev	ypo
error: C:\dev	ypo is not there; a hub that cannot be read is not an empty hub
```

An empty reading names no stage, and what no reading names is struck - so a mistyped path, or a shell that left a variable unexpanded, would arrive as an instruction to strike every version of the project. A hub that holds only its handwritten texts is still a hub and is read as usual.

## As JSON

Reading a hub prints its report.

<!-- json: import -->
| Field | Type | Meaning |
| --- | --- | --- |
| `project` | string | The name of the project. |
| `versions_added` | integer | Versions recorded for the first time. |
| `versions_updated` | integer | Versions that changed. |
| `versions_dropped` | integer | Planned versions struck because the hub no longer names them. |
| `tasks_added` | integer | Tasks recorded for the first time. |
| `tasks_updated` | integer | Tasks that changed. |
| `tasks_dropped` | integer | Tasks struck because the plan no longer lists them. |
| `decisions_added` | integer | Decisions recorded. |
| `questions_added` | integer | Questions for the owner recorded. |
| `questions_withdrawn` | integer | Questions withdrawn because the hub no longer lists them. |
| `wishes_added` | integer | Wishes taken out of the wishes file. |
| `diary_added` | integer | Diary entries recorded. |
| `diary_updated` | integer | Diary entries that changed. |
| `prose_files` | integer | Files whose between-stage prose was taken in. |
| `documents_added` | integer | Handwritten texts taken in as documents. |
| `documents_updated` | integer | Documents replaced by a changed file. |
| `untouched` | array | Generated files not read back, because they are still what the record last wrote. |
| `untouched[]` | string | One file's name. |
| `warnings` | array | Things the reading noticed and went on from. |
| `warnings[]` | string | One warning. |

With `--answers` it prints what was recorded.

<!-- json: import -->
| Field | Type | Meaning |
| --- | --- | --- |
| `project` | string | The name of the project. |
| `added` | integer | Decisions and wishes recorded on this run. |
| `already_recorded` | integer | Entries that were in the record already. |

With `--answers --check` it prints what would be taken.

<!-- json: import -->
| Field | Type | Meaning |
| --- | --- | --- |
| `project` | string | The name of the project. |
| `would_take` | array | The entries that would be recorded. |
| `would_take[]` | object | One entry. |
| `would_take[].kind` | string | `decision` or `wish`. |
| `would_take[].body` | string | The text it would be recorded with. |

## Related

- [`project`](/rigger/reference/project/) - record the project before importing its hub.
- [`backup`](/rigger/reference/backup/) - copy the database aside before a large import.
- [`doctor`](/rigger/reference/doctor/) - what the record holds afterwards.
