# The cases somebody decided, kept where the rule is.
#
# Rego's own test format, so a case here is one an `opa test` reader recognises.
# `yidam policy test` runs every `test_*` rule and treats **undefined as a
# failure**: a body that did not hold asserted nothing, and counting that as a
# pass is how a suite comes to cover less than it claims.
package yidam.disclose.record_test

import data.yidam.disclose.record

# A record that says nothing about redistribution is not a licence.
test_silence_is_not_a_licence if {
	d := record.decision with input as {
		"repo": {"private_paths": []},
		"subject": {"rel": ".yidam/catalog/pearl-2009.md"},
	}
	not d.allow
	some x in d.deny
	x.rule == "unstated-redistribution"
}

test_an_explicit_licence_permits if {
	d := record.decision with input as {
		"repo": {"private_paths": []},
		"subject": {"rel": ".yidam/catalog/x.md", "redistributable": true},
	}
	d.allow
}

# Reported as a licence, not as an omission. Somebody who wrote
# `redistributable: false` does not need to be told to add the field.
test_an_explicit_refusal_is_a_licence_and_not_an_omission if {
	d := record.decision with input as {
		"repo": {"private_paths": []},
		"subject": {"rel": ".yidam/catalog/x.md", "redistributable": false},
	}
	some x in d.deny
	x.rule == "not-redistributable"
}

# **The load-bearing case.** A declared-private path refuses a push the licence
# would have allowed, and it is reported *instead of* the licence finding.
test_a_private_path_beats_a_licence if {
	d := record.decision with input as {
		"repo": {"private_paths": [{"path": "dossier", "holds_content": true}]},
		"subject": {"rel": "dossier/evidence.md", "redistributable": true},
	}
	not d.allow
	count(d.deny) == 1
	some x in d.deny
	x.rule == "private-path"
}

# The trap: `dossiers` is a different directory whose name merely starts the same.
test_a_name_prefix_is_not_a_directory_prefix if {
	d := record.decision with input as {
		"repo": {"private_paths": [{"path": "dossier", "holds_content": true}]},
		"subject": {"rel": "dossiers/a.md", "redistributable": true},
	}
	d.allow
}

test_the_declared_path_itself_is_covered if {
	d := record.decision with input as {
		"repo": {"private_paths": [{"path": "dossier", "holds_content": true}]},
		"subject": {"rel": "dossier", "redistributable": true},
	}
	not d.allow
}

# A record is judged on its path, not on what sits beside it — `is_private` never
# consulted the filesystem and this inherits that.
test_a_placeholder_flag_does_not_license_a_record if {
	d := record.decision with input as {
		"repo": {"private_paths": [{"path": "dossier", "holds_content": false}]},
		"subject": {"rel": "dossier/x.md", "redistributable": true},
	}
	not d.allow
}
