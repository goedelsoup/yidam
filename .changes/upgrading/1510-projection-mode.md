### A generated corpus can declare itself a projection

**A corpus written by a program can now say so in `.yidam/config.toml` (#1510).**

Add a `[corpus]` table with `mode = "projection"` and `generated_by`, naming the producer.
The producer also writes `.yidam/corpus/corpus.toml` with an `id` on every run.
That id names the corpus in RDF subjects and vector keys, in place of the genesis hash.
Keep the id stable across runs, since a new id re-keys everything published.

In a projection, commands that write the corpus refuse and name the producer.
`doctor` skips its vendoring checks, and `status` reports `mode` and no genesis date.

Two changes apply to every corpus.
A class may declare `outgoing: optional` to exempt its instances from `orphan-out`.
Every command that reads a corpus takes `--root`, and `YIDAM_ROOT` names one when `--root` does not.

Nothing is owed by an authored corpus. See [RFC-0064](rfcs/0064-a-projected-corpus.md).
