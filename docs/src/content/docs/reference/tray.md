---
title: tray
description: A card's tray - the material that arrives for a task, and where it goes once sorted.
---

```
rigger tray show [<CARD>] [--json]
rigger tray edit [<CARD>]
rigger tray fetch [<CARD>] [--from <PATH>]... [--max-mb <MB>] [--video] [--json]
rigger tray intake [<CARD>] [--since <WINDOW>] [--max-mb <MB>] [--video] [--json]
rigger tray done [<CARD>] [--json]
rigger tray list [--json]
```

A ticket arrives with more than its text: screenshots, an export, a log a tester left in a shared folder, and what the person handing it over knows about where it is worked. The **tray** is where that lands - a folder per [card](/rigger/reference/task/), with a form in it for what only a person can say, and the files beside the form.

`<CARD>` is a card's key, one of its aliases, or its id; every command takes the card **in hand** when it is left out, and says how to name one when no card is.

The folder is the truth about a tray. Nothing about it is written into the record: every screen that shows a tray reads the folder afresh, the way a card's branches are read from git - so a file dropped in by hand is as much in the tray as one `fetch` copied.

## Where trays are

`<trays>/<KEY>/`, where `<trays>` is what the [profile](/rigger/reference/profile/) names in `trays`, or `trays/` in the profile's own directory. The key is the card's; a card [renamed](/rigger/reference/task/#making-and-keeping-a-card) when its tracker names it keeps its old key as an alias, and a tray made under that key moves to the new one the first time it is looked for - there is one tray per task.

```
trays/ACME-7310/
  incoming.md          the form
  shot-142229.png      what waits to be sorted
  logs/app.log
  sorted/2026-09-20/   a round sorted before, with the form as it was filled
```

## `tray show`

```console
$ rigger tray show
ACME-7310 · rtf files are not supported
  made the tray, with a blank form to fill in
  tray:  C:\Users\you\AppData\Local\lacodda\rigger\data\profiles\work\trays\ACME-7310
  form:  C:\Users\you\AppData\Local\lacodda\rigger\data\profiles\work\trays\ACME-7310\incoming.md (blank)
  nothing waits in it
```

The first time, the tray is made with a blank form; after that, the form says whether it is **filled** - whether any line in it is an answer rather than a heading or one of the form's own `>` lines - and the files are listed oldest first, with their size and the local time they were written: the time a screenshot was taken is what matches it to "reproduced at 14:22". `tray edit` makes the tray if it has to and opens the form in the same editor as [`doc edit`](/rigger/reference/doc/).

### The form

| Section | What goes in it |
| --- | --- |
| Where the work is done | the directories that may be changed, one per line; several copies of a project are often checked out, and a folder's name does not say which branch it holds |
| The task as written | the ticket's text, pasted as it is |
| What was reproduced | did, got, expected, the version and the stand, and the time - which is what finds the place in a log |
| Links to files | folders a tester left material in, one per line; what [`fetch`](#tray-fetch) takes from when it is not told |
| Done before | commits, branches, words to search the history for |
| Compare against | where to see how it should be |
| From colleagues | hints and answers from neighbouring teams, quoted with who said them |
| What the files show | a line per file, when there are many alike |

The lines that start with `>` explain each section and stay in the form: they are there again next time.

## `tray fetch`

```console
$ rigger tray fetch --from \\fileserver\testers\2026\0915\3
Into the tray of ACME-7310 from \\fileserver\testers\2026\0915\3:
  took shot-142229.png  212 KB
  took logs/app.log  48 KB
  left \\fileserver\testers\2026\0915\3\screen.mp4 (31.4 MB): a recording; describe what it shows, or take it with --video
Took 2 files (260 KB); left 1 file where it is
  tray: C:\...\trays\ACME-7310
```

Copies a folder's files - or one file - into the tray, keeping the folder's layout under it and the moment each file was written. Without `--from` it takes every folder the form lists under **Links to files**, so filling the form is all a tester's material needs.

What is not copied is always said, with where it is, so it can be worked with in place:

| Left behind | Why |
| --- | --- |
| a recording (`mp4`, `avi`, `mkv`, `mov`, `webm`, `wmv`) | an assistant cannot read one; `--video` takes it anyway |
| a file over `--max-mb` (25 by default) | a thirty-megabyte log archive is unpacked where it is, not copied to be unpacked again |
| a file of the same name that is a different file | the tray's copy is kept; nothing in a tray is overwritten |

A file already in the tray is not copied twice, so fetching the same folder again takes only what is new. A folder's files about itself - `desktop.ini`, `Thumbs.db`, `.DS_Store` - are nobody's material and are passed over without a word. A share is slow - tens of megabytes take minutes - and each file is printed as it lands, so a long fetch is seen to be moving.

## `tray intake`

```console
$ rigger tray intake --since 4h
Into the tray of ACME-7310, what was written in the last 4h in C:\Users\you\Pictures\Screenshots, C:\Users\you\Downloads, C:\Users\you\Desktop:
  took Screenshot 2026-09-25 142229.png  212 KB
  took export.csv  3 KB
Took 2 files (215 KB)
```

Takes what was written lately in the places a person's own material lands: the screenshots folder, the downloads and the desktop, as the system names them - moved by the owner or not. `--since` is `4h` by default, and takes `90m`, `1d`, `2w`, or a bare number of hours. The folders are not walked into: a folder in the downloads was put there on purpose, and is fetched by name. What lands there without being anybody's material is passed over without a word: a shortcut an installer left on the desktop (`.lnk`, `.url`), a download still under way (`.crdownload`, `.part`, `.partial`, `.download`, `.tmp`), and a folder's files about itself.

`RIGGER_INTAKE_DIRS` - a list of directories, separated as `PATH` is - replaces the three, for a person whose screenshots land elsewhere. A scratch record (`RIGGER_DATA_DIR` set) takes from no folder unless it names one there, so a test never reads the downloads of the machine it runs on.

## `tray done`

```console
$ rigger tray done
Sorted the tray of ACME-7310: 3 entries moved to C:\...\trays\ACME-7310\sorted\2026-09-25; the filled form went with them, and the tray's is blank again
```

Sorting is over: everything in the tray moves into `sorted/<day>/` beside it - the card's archive - a filled form goes with it, and the tray's form is blank again, ready for the next round. The day is the local one. A second round on the same day lands beside the first, with ` (2)` added to a name that is taken: nothing is overwritten. Run it once the material has been read and what mattered has been written against the card - it is a move, and the archive is where to look afterwards.

## `tray list`

```console
$ rigger tray list
ACME-7310  2 files, the form filled   rtf files are not supported
ACME-5     1 file                     Export drops the footer
```

Every tray with something waiting in it: a file, or a form with an answer. A tray whose folder names no card is listed as such.

## On the card

A card with a tray says so on its [screen and in its packet](/rigger/reference/task/#what-waits-in-the-tray): whether the form is filled, what waits, and the rounds sorted before - so a session starts by reading the material rather than by being told about it.

## JSON

`tray show --json`:

```json
{
  "card": { "key": "ACME-7310", "title": "rtf files are not supported" },
  "made": false,
  "path": "C:\\...\\trays\\ACME-7310",
  "form": { "path": "C:\\...\\trays\\ACME-7310\\incoming.md", "filled": true },
  "files": [{ "path": "logs/app.log", "bytes": 49152, "modified": "2026-09-25T17:22:29Z" }],
  "sorted": [{ "day": "2026-09-20", "files": 5 }]
}
```

`fetch` and `intake` give `card`, `path`, `from` (where they took from), `taken` (as `files` above) and `skipped` - each with its `path`, `bytes` and `why`: `video`, `too-big`, `present` or `differs`. `done` gives `card`, `moved`, `archive` and `form` (whether a filled form went with it). `list` gives, per tray, `key`, `title` and `status` of its card (`null` when no card has that name), `path`, `files` and `form_filled`.

## Related

- [`task`](/rigger/reference/task/) - the cards the trays belong to.
- [`profile`](/rigger/reference/profile/) - `trays`, where they are kept.
