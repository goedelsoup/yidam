# May this computed artifact be uploaded?
#
# Transcribed from `vault::derived_may_push`. A catalog artifact is refused for
# what its own record says; a derived one has no record, because nobody fetched
# it, so the question is answered from what it was built out of:
#
# > An index is not a file that happens to sit in `.yidam/index/`; it is a
# > re-encoding of everything walked to build it.
#
# `subject.sources` is `derived_sources(kind)`, supplied by the binary that owns
# the bundle. That is the half of #443 this fixes at the root: the release
# workflow used to keep its own copy of that list and the copy was wrong.
package yidam.disclose.derived

# **Calls into `lib` are fully qualified, and must stay that way.**
#
# `import data.yidam.disclose.lib` followed by `lib.under(...)` resolves for a
# *rule* and not for a *function*: this engine reports `could not find function
# lib.under` at evaluation, which is to say when a decision is needed. Measured
# against regorus 0.11 in every form — bare import, `as` alias, importing the
# function itself. Tidying these into an import is a change that compiles, passes
# `policy check`, and fails the first time somebody pushes.

decision := {"allow": count(deny) == 0, "deny": deny}

# One denial per declared path that intersects a source directory.
#
# `derived_may_push` returns the first match; this reports every one. That is a
# deliberate improvement rather than a divergence — somebody about to fix this
# wants all the paths, not the alphabetically first — and the equivalence test
# pins it as such: the verdicts agree, and the Rust message is among these.
deny contains d if {
	some p in data.yidam.disclose.lib.with_content(input.repo.private_paths)
	some src in input.subject.sources
	data.yidam.disclose.lib.either_contains(src, p)
	d := {
		"rule": "derived-from-private",
		"msg": sprintf(
			"`%s` is declared private and is part of what the %s is built from (%s). The %s carries the text of every node it encodes, so pushing it would publish that material — and the artifact outlives the access",
			[p, input.subject.kind, src, input.subject.kind],
		),
	}
}
