# ADR 0010: A line of a plan is matched by what it says

- Status: accepted
- Date: 2026-11-06

## Context

A hub has no ids, so reading a plan has to decide which task each line is. It matched by text within the stage and then by position, and a stage was the row with that number. A number is not a stage: one stage inserted moves every stage behind it up. The tasks on a number then took the words of the stage standing there now, closed tasks became closed strangers, and what did not fit stayed behind as duplicates. A generated plan struck nothing, so its duplicates stayed for good. Three neighbours ran into this on real plans.

## Decision

Every line naming a task of its own stage claims it first, across the whole reading. Only then is anything weaker tried, in this order:

1. Its text in another stage still to come, not claimed and not struck. The task moves, keeping its id and history.
2. Its position, while the stage kept its title.

A line neither places is a new task. Striking waits until every line is placed, because a task is carried out of a stage that no longer lists it, and striking that stage first would strike what was about to leave.

A stage still to come that came back under another title is treated as another stage on the number: no matching by position, and what is left under it is struck, even from a generated plan. A generated plan still strikes nothing else, since a stage or a task added after the export is not listed either, except a stage emptied line by line.

A stage's number stays its identity. The stage is not matched by its title across numbers: nothing hangs off a stage row on a plan except a few links, and following it would mean renaming rows in a chain to read a file. Renumbering where identity is needed is `rigger version move`, which keeps the row.

## Consequences

- A wrong match is impossible where the plan gives no evidence for it, and a missed one is visible: a reworded line in a stage that was also retitled arrives as a new task, with the old one struck. That loses its history, and a status given to a stranger would lose more without showing.
- The same words in two stages stay two tasks, which real plans rely on.
- A task that shipped or was struck is never carried by its words.
- Residue of the old reading, the same line in two neighbouring stages, is not cleaned up by this: it reads as two stages naming their own lines. `doctor` naming it is planned.
