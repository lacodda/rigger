# A refactoring turn

Code that is only ever added to gets slower to change. Every so often a project's turn goes to making it easier to work on instead of adding to it. `rigger refactor` says whose turn that is, one at a time across the line. This ritual says how.

A refactoring changes how the code is built and leaves what it does exactly as it was. That is the whole definition, and the reason the rest of this is safe to do in one sitting. If the behaviour moves, it was something else, and it goes in another commit.

It takes one turn and makes one tag. It does not try to fix the whole project: a theme, measured, finished, and the rest written down for next time.

## 1. Measure first

Numbers before any edit: a refactoring with no number before it cannot show it did anything, and it will be argued about by feel. Keep them; they go in the release notes.

- Size: the largest files and functions.
- Change: which files change most often (below).
- Tests: how long the suite takes, how many there are, the mutation score where one exists, flaky ones.
- Build: time from clean and incremental, the size of what ships.
- Runtime: start time and the slowest common operations; the slowest queries, with their plans.
- Warnings, unused dependencies, dead code the tools can name.

## 2. Choose by where it hurts

Most code is rarely touched, and there it can stay ugly at no cost. What costs is code that is both complicated and often changed: that is where effort pays back, and where most defects turn up. Rank by the two together.

```bash
git log --since="6 months ago" --format= --name-only | sort | uniq -c | sort -rn | head -30
```

Cross that with size or complexity. Read the top of the list, then pick **one to three zones** that form a theme. Name it in a line. Everything else you notice goes to `wish`, and does not get done now.

Not a reason on its own: it is old, it is unfashionable, it would be nicer in another style.

## 3. Pin down what it does today

Before touching a zone, make sure something fails when its behaviour changes.

- Where tests are thin, write characterisation tests: tests of what the code does now, right or wrong. They are not a statement that it is correct. A bug they turn up goes in a `fix` commit of its own or to `wish`, and is not smuggled in.
- Break the zone on purpose in a few places and see the tests fail. A green suite that survives a broken guard guards nothing.
- For a public surface, keep real output from before: run the commands on real data and save what they print, to compare at the end. It should match byte for byte, except where it is meant to.

## 4. Change it in small steps

One hat at a time. While refactoring you do not add features and do not fix bugs. A change that needs both is two commits.

- One mechanical move to a commit, named `refactor(scope): ...`, with the gate green after each. Then a mistake costs one revert rather than an afternoon.
- If something goes red and it is not obvious why within a few minutes, revert the step. Going back is cheap here; that is what the small steps bought.
- The tool does the moving where it can: the compiler's errors, the language server's rename.

What to reach for, roughly in order of how much it usually pays:

- **Split what does too much.** A file everyone edits for different reasons; a function with several jobs. Give each piece a name that says what it is for.
- **Say it in the domain's words.** Rename until the code reads like the product talks.
- **Make the wrong state impossible to write.** A type instead of a string and a comment; one source of truth instead of two copies to keep in step.
- **Delete.** Dead code, options nobody passes, layers with one caller, abstractions for a second case that never came. Deleting is the cheapest maintenance there is.
- **Merge what is really one thing**, on the third copy rather than the second: two similar pieces may be a coincidence.
- **Move the slow part out of the way**, measured, rather than optimising what looks slow.

Do not rewrite. A rewrite throws away everything the old code learnt the hard way and stops the product while it happens. Replace piece by piece, with both running where they must.

## 5. The tests themselves

Tests are code, and they rot the same way, but they are also what makes this safe, so change them apart from the code they hold: not in the same commit.

- Delete a test that cannot fail. Merge those that check one thing many times.
- Make the slow ones fast, and name flaky ones and fix them: a suite people wait for or do not trust stops protecting anything.
- Say what is being protected in the test's name.
- Never lower a floor to get through this.

## 6. The database

Data outlives code, so the bar is higher here, and the order matters.

- Read the plan of the slowest queries (`EXPLAIN QUERY PLAN`, `EXPLAIN ANALYZE`) before changing an index. Add what is missing; drop what no query uses, which costs on every write.
- Look for a query per row in a loop.
- Let the schema carry the rules: constraints, not-null, foreign keys, the right types. A rule held only by code will one day be broken by some other code.
- Look for what is stored twice and can disagree.
- A migration is rehearsed on a copy of real data first, with a backup taken, and timed. An applied migration is frozen: a mistake gets a new one.
- When something else still reads the old shape, expand first and contract later: add the new, write both, move the readers, remove the old, as separate releases.
- Compare before and after on real data: row counts, and what the screens show.

## 7. Dependencies and the build

Remove what is unused. Replace a heavy dependency that is used for one small thing. Trim features. Check the build time and what ships against the first measurement.

## 8. What does not move

The contracts others rely on stay still: command names and flags, JSON fields, file formats, APIs, the database a user already has. Anything observable can be depended on by someone. A contract worth changing is a feature with its own version, and a line in `wish`.

## 9. Measure again, and decide

Run the same measurements and compare.

- **It improved:** the table of before and after goes in the release notes. Compare the output saved in step 3.
- **It did not:** revert it. Clearer by feel and worse by number is not a win to ship. Say so in the record, so nobody tries the same thing next time without a new reason.
- **Nothing here is worth doing:** that is a good answer. Do not invent work. Record what was measured with `refactor done` and take the next stage instead.

## 10. Finish

- The gate, the release: a patch of the last release, since nothing a user can see has changed.
- `rigger refactor done <project> "<theme: what was measured, before and after>"`. That starts the count to its next one, and frees the week's slot only next week.
- Whatever was left: `wish`, with where it hurts.
