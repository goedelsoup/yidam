# samudaya examples

*Seed material, for reading and for copying. **Not** seeds of this repository.*

Two different things live here.

| | What it is | Read it to learn |
|---|---|---|
| `axiom.md`, `hint.md`, `constraint.md`, `augmentation.md` | One stub per `kind`, with the frontmatter and the shape of a body | The **format** |
| `genealogy/`, `museum-provenance/`, `language-documentation/` | Complete seed sets for one domain each | What a seed set **says** |

The stubs answer "what does the file look like". They have never seeded anything, and a
reader following [quickstart](../../docs/quickstart.md) step 4 — *"write it into `samudaya/`
before you start"* — got four schema templates and no example of a seed set that had ever
produced an ontology. The domain sets are that missing example.

## Using one

A seed set is input to a bootstrap, so it goes in the repository being bootstrapped, not this
one:

```sh
yidam clone ../my-genealogy
cp -R samudaya/examples/genealogy/*.md ../my-genealogy/samudaya/
cd ../my-genealogy && yidam samudaya-audit    # 5 seeds, across 4 kind(s)
```

Then run the bootstrap. It reads `samudaya/`, folds the axioms and hints into the
ontology-discovery dialogue, enforces the constraints while scaffolding, and commits the
removal of `samudaya/` as an explicit consumption event.

Copy the ones that are true of your work and delete the rest. A seed you did not mean is
worse than a seed you did not write: the dialogue will argue for it, and you will have to
argue back.

## Why nothing here is live

`samudaya/` in *this* repository is empty of seeds, and must stay that way. Both consumers —
`count_seeds` in `yidam/cli/src/cmd/overlay.rs` and `yidam samudaya-audit` — skip exactly one
subdirectory, `examples/`, and nothing else. A domain set placed anywhere else under
`samudaya/` would therefore be read as a **live seed of this repository**: `overlay` would
copy `samudaya/` into an existing repository, and every repository derived from this template
would report three other domains' axioms as seeds for its bootstrap to consume at genesis.

Here, both exclusions already cover them. `count_seeds` stays at zero, `overlay` keeps
printing *"samudaya/ has no seeds — skipping"*, and a derived repository is born with its own
ontology and nobody else's axioms.

Note what is *not* the mechanism. `yidam clone` copies `samudaya/` across either way — it
excludes only top-level `docs/` and `examples/`, and this is neither — so a derived repository
does receive these sets. That is harmless and deliberate: it inherits the samudaya
documentation, its audit reports zero seeds, and the whole directory is destroyed at genesis.
What placement decides is whether the new repository reads them as *its own*.

The cost of that placement is that `samudaya-audit` will not validate these either — it skips
the directory they are in. `yidam/cli/tests/samudaya_examples.rs` is what checks them
instead, holding them to the same `kind` vocabulary the audit enforces, read from the audit's
own list rather than from a second copy of it.

## What a seed set is not

Everything [`samudaya/README.md`](../README.md) says, and one thing worth repeating because
it is the mistake these examples are most likely to cause: **a seed set seeds the discovery
loop and does not replace it.** These files state what a domain's practitioners already
commit to before any dialogue happens. They do not state a class list, and the bootstrap is
still required to ask.
