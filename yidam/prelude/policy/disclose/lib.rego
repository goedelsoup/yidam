# The predicates every disclosure decision shares.
#
# Until RFC-0024 these existed four times: twice inlined in shell
# (`ci.yml`, `release.yml`) and twice in Rust (`is_private`, and the
# intersection inside `derived_may_push`). Two of them disagreed about what
# "under a declared path" means, deliberately, and nothing named the
# difference. Naming it is most of what this file is for.
package yidam.disclose.lib

# A record's own path sits under a declared path.
#
# **One-directional**, and that is the whole distinction from `intersects`. A
# catalog record is a single file: the question is whether that file is inside
# something declared private. `dossier` covers `dossier/a/b.md`; it does not
# cover `dossiers/x.md`, which is a different directory whose name merely starts
# the same way.
under(rel, declared) if {
	some p in declared
	prefix_match(trim_prefix(rel, "./"), p)
}

prefix_match(rel, p) if rel == p

prefix_match(rel, p) if startswith(rel, sprintf("%s/", [p]))

# A declared path and a source directory overlap, in either direction.
#
# **Bidirectional**, because a derived artifact encodes a whole directory rather
# than being one file. Both readings are real and the release workflow tests both:
# `dossier/` sitting inside the corpus is the first, and a repository declaring
# `.yidam/` private is the second — it contains every source there is.
intersects(src, declared) if {
	some p in declared
	either_contains(src, p)
}

either_contains(src, p) if p == src

either_contains(src, p) if startswith(p, sprintf("%s/", [src]))

either_contains(src, p) if startswith(src, sprintf("%s/", [p]))

# The declared paths that actually hold material.
#
# `holds_content` is computed by the binary and arrives in the input, because it
# is a filesystem walk and Rego has no filesystem. That is the right side of the
# line anyway: whether a directory contains a file is a fact, and whether a
# placeholder counts as material is the judgement — and the judgement is here.
#
# A directory holding nothing but its own `README.md` or a `.gitkeep` is a
# placeholder. Refusing on one would make the feature unusable for a repository
# that declared its intent before it had anything to protect, which is the order
# `directories.md` asks people to work in.
with_content(paths) := {p.path | some p in paths; p.holds_content}

# Every declared path, placeholder or not.
#
# `record` uses this and `derived` uses `with_content`, and the asymmetry is
# inherited rather than invented: `is_private` never consulted the filesystem,
# because a catalog record naming a file under a declared path is a statement
# about where that record lives and does not depend on what else is beside it.
all_paths(paths) := {p.path | some p in paths}
