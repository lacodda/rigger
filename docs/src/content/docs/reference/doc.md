---
title: doc
description: The handwritten texts of a project, kept in the record.
---

```
rigger doc list <project> [--kind KIND] [--json]
rigger doc show <project> <slug> [--json]
rigger doc add  <project> <title> [--kind KIND] [--slug SLUG] [--body TEXT|-]
rigger doc edit <project> <slug> [--title TITLE] [--body TEXT|-]
rigger doc remove <project> <slug>
rigger doc template <kind> [--write]
```

A project's plan, changes and diary are written *out of* the record. Its handwritten texts were the opposite: the vision, the prose of the decisions journal, the rituals only that project states about itself, and the research notes lived only as files in a hub. `doc` brings them in, so that the record holds everything and a hub becomes an export rather than a place where half the truth is kept.

## The kinds

| Kind | How many | What it is |
| --- | --- | --- |
| `vision` | one | The product compass: why this exists, and what it is not. |
| `decisions` | one | The prose preamble of the decisions journal. The entries themselves are events, not a document. |
| `rituals` | one | What only this project says about itself, over and above the line's rituals. |
| `research` | many | A note dated to the day it was asked: the question, what was found, what was decided. |
| `other` | many | A text that is none of these - rather than a reason to invent a kind. |

A second document of a singular kind is refused by name, because two visions means nobody reads either:

```console
$ rigger doc add rigger "Another" --kind vision
error: rigger already has a vision: 'vision-of-rigger'; edit it with `rigger doc edit rigger vision-of-rigger`
```

## Listing and reading

```console
$ rigger doc list rigger
3 documents:
  vision-of-rigger                 vision     today        Vision of rigger
  2026-09-04-the-tool-for-projects research   today        2026-09-04 - The tool for projects
  rituals                          rituals    today        Rituals
```

A document is addressed by its **slug**, which is made from the title and then stays put: renaming a document does not move it. `--kind` narrows the list, and `--json` gives the same rows without the bodies, since the vision of a mature project is longer than the screen.

`show` prints the body and nothing else, so it can be redirected into a file:

```console
$ rigger doc show rigger vision-of-rigger > vision.md
```

With `--json` it prints the whole row instead - kind, title, body, and the days it was created and last written.

## Writing

`add` and `edit` open `$EDITOR` and take back whatever was saved:

```console
$ rigger doc add rigger "Vision of rigger" --kind vision
```

A new document does not start blank. It starts from the questions its kind exists to answer, because a blank file is the surest way to get a document nobody writes.

### Skeletons of your own

`doc template <kind>` shows the skeleton a kind starts from, and `--write` puts it in the profile's directory as `doc.<kind>.md` for you to edit:

```console
$ rigger doc template vision
# {{title}}

## Why

## The idea

## What it is made of

## Boundaries

## What success looks like

(the built-in skeleton; `rigger doc template vision --write` to make it yours)
```

`{{title}}` in the file becomes the document's title. What rigger ships is in English, because everything it ships is - the headings you actually write under are yours, and this is how you say so without patching the binary. A kind with no file of its own keeps the skeleton that ships, and deleting the file goes back to it.

The profile's directory comes first, then the data directory shared by every profile: a line of products and a ticket desk do not write the same kind of vision.

`--body` skips the editor, and `--body -` reads standard input, which is how a script or an assistant writes one:

```console
$ rigger doc add rigger "Notes" --body -
```

Leaving the editor without writing anything makes no document. On `edit`, emptying an existing one is refused: removing a document is `doc remove`, and it should be said rather than implied.

`--title` on its own renames a document without opening an editor. A rewrite keeps the day the document was started; only its `updated_at` moves.

### Which editor

In order: `RIGGER_EDITOR`, then `VISUAL` or `EDITOR`, then [scheda](https://github.com/lacodda/scheda) when it is installed, then the system default (`notepad` on Windows, `vi` elsewhere).

scheda is the line's own markdown notepad, and it takes `--wait FILE`: the process stays alive until the tab is closed and gives back its status, which is the contract an `$EDITOR` has to meet. Preferring it when it is there means a vision is edited in the editor written for exactly that, with nothing to configure. Someone who has set `EDITOR` has said what they want, and that still wins.

The text is edited through a file in a directory of that rigger process's own, under the temporary directory, and both are removed afterwards whether the edit succeeded or not: the file holds your prose, and leaving copies of it lying about is not something a record tool should do. Per process, because two rigger runs editing a document of the same name - two projects each with a `vision`, or two sittings at once - would otherwise share one scratch file, and whichever saved second would win.

## Related

- [`note`](/rigger/reference/note/) - the events a decisions journal is made of.
- [`export`](/rigger/reference/export/) - writing a hub back out of the record.
- [`import`](/rigger/reference/import/) - reading a hub into it.
- [`backup`](/rigger/reference/backup/) - the copy that matters more once the texts live here.
