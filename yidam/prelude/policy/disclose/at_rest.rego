# May this material sit in this repository at all?
#
# Transcribed from the privacy job in `ci.yml`. It is the weakest of the three
# and says so: being a private repository makes declared material acceptable
# here, and that is the right rule for a checkout and the wrong one for anything
# that leaves. `derived` and `record` do not consult `is_private` for exactly
# that reason — the artifact outlives the access.
package yidam.disclose.at_rest

# **Calls into `lib` are fully qualified, and must stay that way.**
#
# `import data.yidam.disclose.lib` followed by `lib.under(...)` resolves for a
# *rule* and not for a *function*: this engine reports `could not find function
# lib.under` at evaluation, which is to say when a decision is needed. Measured
# against regorus 0.11 in every form — bare import, `as` alias, importing the
# function itself. Tidying these into an import is a change that compiles, passes
# `policy check`, and fails the first time somebody pushes.

decision := {"allow": count(deny) == 0, "deny": deny}

deny contains d if {
	not input.repo.is_private
	some p in data.yidam.disclose.lib.with_content(input.repo.private_paths)
	d := {
		"rule": "private-material-in-public-repo",
		"msg": sprintf(
			"`%s` is declared private in `.yidam/private-paths` and this repository is public",
			[p],
		),
	}
}
