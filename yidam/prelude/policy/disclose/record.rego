# May these bytes be uploaded, given what their record says?
#
# Transcribed from `vault::may_push`. **Whether, not where** — routing is
# `Vaults::route`'s question and no rule here reads a vault name, because the two
# fail differently: a route is edited casually by somebody reorganising storage,
# and a licence is not something that edit is allowed to undo.
package yidam.disclose.record

# **Calls into `lib` are fully qualified, and must stay that way.**
#
# `import data.yidam.disclose.lib` followed by `data.yidam.disclose.lib.under(...)` resolves for a
# *rule* and not for a *function*: this engine reports `could not find function
# lib.under` at evaluation, which is to say when a decision is needed. Measured
# against regorus 0.11 in every form — bare import, `as` alias, importing the
# function itself. Tidying these into an import is a change that compiles, passes
# `policy check`, and fails the first time somebody pushes.

decision := {"allow": count(deny) == 0, "deny": deny}

private if data.yidam.disclose.lib.under(input.subject.rel, data.yidam.disclose.lib.all_paths(input.repo.private_paths))

# The private-path refusal, and it is reported instead of the licence one rather
# than beside it.
#
# The precedence is inherited from `may_push`, which returns on the first match,
# and the reason is worth keeping: this is a statement about *this repository*
# that the person running the command can act on, while `redistributable` is a
# fact about a third party's licence they may not be able to change at all.
# Telling somebody their licence is wrong when the actual problem is a path they
# control sends them to argue with a publisher.
deny contains d if {
	private
	d := {
		"rule": "private-path",
		"msg": sprintf(
			"%s is under a path `.yidam/private-paths` declares private. The artifact outlives the access",
			[input.subject.rel],
		),
	}
}

# An explicit refusal, reported as a licence and not as an omission. Somebody who
# wrote `redistributable: false` does not need to be told to add the field.
deny contains d if {
	not private
	input.subject.redistributable == false
	d := {
		"rule": "not-redistributable",
		"msg": sprintf("%s records `redistributable: false` — licensed to read, not to host", [input.subject.rel]),
	}
}

# **The default is refusal.** A record that says nothing about redistribution is
# not a licence, and the first `vault push` anybody runs must not be one — a
# catalog is full of papers, which is exactly the material where that matters.
deny contains d if {
	not private
	not is_boolean(input.subject.redistributable)
	d := {
		"rule": "unstated-redistribution",
		"msg": sprintf(
			"%s does not say whether these bytes may be redistributed. Add `redistributable: true` to the record if they may",
			[input.subject.rel],
		),
	}
}
