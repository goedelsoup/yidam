### The plugin's launcher honours `YIDAM_ROOT`

**Set `YIDAM_ROOT` to serve one corpus in every project (#1520).** The Claude Code plugin's
launcher served the git toplevel only, and refused any project without a `.yidam/`. It now
serves the directory `YIDAM_ROOT` names, as every other command that reads a corpus does. Set
it in the shell or in Claude Code's settings `env`. Unset or empty, nothing changes.

A `YIDAM_ROOT` that is not a directory, or holds no `.yidam/`, is refused by name. Before, the
launcher ignored the variable, so a session could be refused while the variable named a corpus.

No variable needs threading through by hand. The launcher `exec`s the server with its whole
environment, `FASTEMBED_CACHE_DIR` and the `HF_*` variables included. Since #1516 the model
cache is under the corpus root by default, so a shared corpus finds its own cache.

The plugin is version 0.3.1.
