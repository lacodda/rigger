---
title: find
description: Search the record across every project at once.
---

```
rigger find <QUERY> [--project <NAME>] [--kind <KIND>] [--limit <N>] [--json]
```

Searches every event of every project - decisions, findings, pitfalls, and the changes read out of commit messages - and every [document](/rigger/reference/doc/): the visions, the rituals, the research notes. It exists because a record that has been accumulating for a year is past the point where anyone remembers where a thing was settled, and re-deciding something already argued through is the expensive kind of forgetting.

```console
$ rigger find budget
sample       2026-09-04  decision  The budget is a gate, not a suggestion.
sample       2026-09-04  pitfall   A wide window hides what the budget dropped.
```

Each line shows the text around the match, not the start of the event. A decision in a real record runs to fifteen hundred characters and states its subject in a heading, so the first line often does not contain the word searched for - and a result you cannot see the reason for reads as a wrong result.

## What ranks first

What a person wrote outranks what a commit message said, at equal relevance. A commit can always be read again in git; a decision or a pitfall exists nowhere else, and the answer to "where did we decide this?" must not arrive under three commits that happen to share a word.

Both are searched, though - "when did we fix that?" is as real a question as "where did we decide that?".

## Narrowing

```console
$ rigger find budget --project sample --kind decision
2026-09-04  decision  The budget is a gate, not a suggestion.
```

The project column disappears when the search is for one project. `--kind` takes any kind an event can have: `decision`, `finding`, `pitfall`, `change`, `question`, `wish`.

A project name that does not exist is refused rather than answered with nothing - an empty result would send you looking for the wrong thing.

## How a query is read

A bare word is searched as a prefix. Russian inflects and SQLite has no stemmer for it: searching a real record for `бюджет` found two events where `бюджет*` found eight, because the word appears as `бюджета`, `бюджету`, `бюджетом`. English loses nothing by the same rule.

Anything using [FTS5 syntax](https://www.sqlite.org/fts5.html#full_text_query_syntax) is passed through untouched, so the full language stays available:

| Query | Means |
| --- | --- |
| `budget` | any word starting with "budget" |
| `two words` | both, each as a prefix |
| `"exact phrase"` | those words, in that order |
| `packet AND budget` | events with both |
| `packet NOT commit` | one without the other |

When nothing matches, that is a result rather than a failure:

```console
$ rigger find zzz
Nothing matches "zzz".
A bare word is searched as a prefix: try a shorter one, or FTS5 syntax - "exact phrase", `one OR two`.
```

## Documents

A search answers over two kinds of thing, and says which is which. Documents come under the events, each with the command that opens it:

```console
$ rigger find "the record is the truth"
sample       2026-09-04  decision  The record is the database, and a hub is a view of it.

Documents
sample       2026-09-15  rituals   …the record is the truth for everything a hub shows…
             rigger doc show sample rituals
```

A decision and a vision are different kinds of answer, and one that has to be opened with a second command should say so rather than look like another line of the list above it. A title match outranks a body match of equal relevance: someone searching for `vision` means the document called that, not the sentence in the middle of a research note that uses the word.

`--kind` is a question about events, so it leaves documents out - a vision matching the word would be an answer to a question nobody asked.

## Where the answers come from

FTS5 matches words. The question a person actually asks the record - "where did we settle this?" - is about meaning, and the decision that answers it may not contain the word that was typed.

So the search is written against a provider rather than against SQLite, and the semantic index the line is building will drop in behind the same commands. FTS5 stays whatever else arrives: it is exact where meaning is fuzzy, and a search for an error message or a version number wants exactly that.

## Related

- [`why`](/rigger/reference/why/) - the events behind one version, rather than one word.
- [`context`](/rigger/reference/context/) - the recent events, without searching.
- [`note`](/rigger/reference/note/) - how the searchable events get there.
- [`doc`](/rigger/reference/doc/) - the documents a search now reaches.
