# The cases for the rule beside this file. `yidam lint` runs them from the genesis commit,
# and a failing or missing case is reported by `domain-article-unproven`.
package constitution.identity_needs_two_lines_test

import data.constitution.identity_needs_two_lines as article

test_two_tips_pass if {
	count(article.deny) == 0 with input as {"sangha": {"resolutions": [
		{"file": ".yidam/sangha/resolutions/a.md", "tips": ["ma/x@1", "ma/y@2"]},
	]}}
}

test_one_tip_is_refused if {
	some msg in article.deny with input as {"sangha": {"resolutions": [
		{"file": ".yidam/sangha/resolutions/a.md", "tips": ["ma/x@1"]},
	]}}
	contains(msg, "resolutions/a.md reads 1 tip(s)")
}

# A repository with no resolutions has nothing to refuse, which is not the same as a rule
# that failed to answer.
test_no_resolutions_refuse_nothing if {
	count(article.deny) == 0 with input as {"sangha": {"resolutions": []}}
}
