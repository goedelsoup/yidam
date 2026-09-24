# Glossary

The borrowed words the rest of the prelude uses without explaining. Read this first — it is
the shortest document here. A trailing `/` is a branch prefix; the rest are directories.

| term | literally | in a yidam repository |
|---|---|---|
| **yidam** | the chosen form, which shapes you back | a repository whose git history *is* the knowledge: files are nodes, links are edges, commits are events |
| **corpus** | a gathered body | `.yidam/corpus/` — the graph. The corpus *is* the yidam, not a description of one |
| **prelude** | what precedes the work | `.yidam/.vendor/prelude/` — inherited unchanged, read-only, never localized |
| **samudaya** | the arising | `samudaya/` — axioms and constraints placed before bootstrap. Heard once, deleted at genesis |
| **sadhana** | practice | `sadhana/` — the shape copied at genesis, consumed by the copying |
| **kuten** | the form a practice takes | what this corpus's work is *for*, selected in `.yidam/decisions/kuten.yml`. Binds nobody |
| **tonpa** | the one who shows | `.yidam/tonpa/` — corpora built by other sanghas, installed as dependencies. Cited, never edited |
| **sangha** | the community holding it | everyone maintaining this domain, human and agent. `.yidam/sangha/` holds its governance |
| **ma/** | one voice | an elector's working position. Held with integrity, not yet recognized |
| **rigpa/** | clear seeing | a position the sangha has recognized. Settled, not argued into |
| **phase/** | | one bounded inquiry on the baseline. The single-elector default |
| **propose/** | | commits `yidam propose` drafted, awaiting a person |

`ma/`, `rigpa/` and `.yidam/sangha/` exist only under `governance: collective`. Single-elector
is the default and the common case; there the baseline is `main` and inquiry runs on `phase/`.

The last two rows are English deliberately: they name procedure, the borrowed words name
standing, and a draft has none to borrow. [GRAPH.md](GRAPH.md) has the encoding,
[SCRIPTURE.md](SCRIPTURE.md) why any of it is named this way.
