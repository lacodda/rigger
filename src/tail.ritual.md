# Growing a plan

Software that is alive keeps being worked on, and a plan that runs out freezes the product. So the plan is grown before it runs out, a little and often. `rigger` says when: at the start of a turn, in the first message and in the packet. This ritual says how. It belongs at the start of the turn; the stage still goes to a release in the same sitting.

## Sort: what has piled up

Every time, grow or sort, first go through what is already asked for. `rigger context <project>` lists the wishes and the neighbours' orders.

- Built already, or no longer wanted: `resolve` it.
- A defect with a diagnosis: into the nearest stage. Do not save it for later.
- Everything else: give it a place in the plan and `resolve` it, or leave it with a reason to wait.

Most of what a plan needs is usually here already. Sort before inventing.

## Grow: when the tail is short

Then, when `rigger tail <project>` says `grow`, add versions until there are several good ones past the floor. Look in this order, and stop when the ideas stop being good:

1. What the project's own use has shown: what was slow, awkward, or done by hand twice.
2. What its Vision promises and it does not do yet.
3. What neighbours keep working around.
4. What comparable products do that its users would miss.
5. Companion programs and whole new surfaces: these are big ideas, below.

Ask for a spread: simplifying what exists, small additions, one that is ambitious. Removing something counts.

## Small or big

**Small goes straight into the plan** with `version add`, one version, one coherent theme. An idea is small when all of this holds:

- it fits in one stage;
- it adds no new surface: no new program, screen area or command family;
- it does not change the model of the data;
- it does not touch what the Vision says the product is;
- it has a source you can name: an order, a finding, a measurement, the owner's words. An idea with no source is an idea for the sake of the tail, and stays out.

With `tail.cap` versions already planned, small ideas stop going in by themselves: sort instead.

**Everything else is big, and goes to the owner** with `rigger ideas add <project> "<text>"`: the idea, why, what it would cost, and its source. Big ideas are asked about together, in one questionnaire across the line, every `ideas.every_days` days. They do not go to the inbox, where they would wait among the questions that hold a stage up. Do not build one before it is answered.

## Finish

`rigger tail done <project> "<what changed: versions added, wishes sorted, ideas raised>"`, and go on to the stage. Then the same project does not ask again this turn.
