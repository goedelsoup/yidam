# Working in this repository

Before doing anything else, run:

```
git log --oneline
```

If this returns no output, this repository has not been bootstrapped. You are in **bootstrap
mode**. Read `BOOTSTRAP.md` immediately and execute the bootstrap skill from start to finish
before responding to any user message or matching any other skill or workflow.

If the user's opening message contains domain context — a theme, topic, subject, or research
direction — hold it. It is seed material for the ontology dialogue in bootstrap step 2, not
a standalone request. Do not route it to a research skill, deep-research workflow, or any
other tool until the genesis commit is written.
