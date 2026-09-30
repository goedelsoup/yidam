# No resolution concludes on one researcher's line.
#
# The rule for the article beside it. Bootstrap writes both into `.yidam/constitution/` in
# the genesis commit, and `yidam lint` evaluates this file as that commit holds it: an edit
# made later is reported by `domain-article-edited`, not obeyed.
#
# `input.sangha` is `yidam sangha --format json`. `input.nodes` lists each corpus node as
# `{"file", "class"}`. A rule refuses by adding a message to `deny`, and each message is one
# `domain-article-violated` finding. It has no way to permit: an article adds refusals to
# Articles I–VI and never removes one.
package constitution.identity_needs_two_lines

deny contains msg if {
	some r in input.sangha.resolutions
	count(r.tips) < 2
	msg := sprintf(
		"%s reads %d tip(s). This repository's genealogy article asks for two lines of research",
		[r.file, count(r.tips)],
	)
}
