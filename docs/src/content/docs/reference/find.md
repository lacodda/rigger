---
title: find
description: Search the record across every project at once.
---

```
rigger find <QUERY> [--project <NAME>] [--kind <KIND>] [--limit <N>] [--json]
```

Searches every event of every project: decisions, findings, pitfalls, and the changes read out of commit messages. It exists because a record that has been accumulating for a year is past the point where anyone remembers where a thing was settled - and re-deciding something already argued through is the expensive kind of forgetting.

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

## Related

- [`why`](/rigger/reference/why/) - the events behind one version, rather than one word.
- [`context`](/rigger/reference/context/) - the recent events, without searching.
- [`note`](/rigger/reference/note/) - how the searchable events get there.
