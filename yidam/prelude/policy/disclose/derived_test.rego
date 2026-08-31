# Cases for the artifact that has no record.
package yidam.disclose.derived_test

import data.yidam.disclose.derived

sources := [".yidam/corpus", ".yidam/catalog"]

# Intersection both ways: a declared path inside a source directory…
test_a_private_path_inside_a_source_refuses if {
	d := derived.decision with input as {
		"repo": {"private_paths": [{"path": ".yidam/corpus/secret", "holds_content": true}]},
		"subject": {"kind": "index", "sources": sources},
	}
	not d.allow
}

# …and a declared path that *contains* one. A repository declaring `.yidam`
# private has declared every source there is.
test_a_private_path_containing_a_source_refuses if {
	d := derived.decision with input as {
		"repo": {"private_paths": [{"path": ".yidam", "holds_content": true}]},
		"subject": {"kind": "index", "sources": sources},
	}
	not d.allow
}

test_no_overlap_permits if {
	d := derived.decision with input as {
		"repo": {"private_paths": [{"path": "dossier", "holds_content": true}]},
		"subject": {"kind": "index", "sources": sources},
	}
	d.allow
}

# **The rule a naive transcription drops.** A declared directory holding nothing
# but a README or a .gitkeep is a placeholder, and refusing on it would make the
# feature unusable for a repository that declared its intent before it had
# anything to protect.
test_a_placeholder_directory_does_not_refuse if {
	d := derived.decision with input as {
		"repo": {"private_paths": [{"path": ".yidam/corpus", "holds_content": false}]},
		"subject": {"kind": "bundle", "sources": sources},
	}
	d.allow
}

# Every overlapping path is named, not the first. `derived_may_push` could only
# ever report one, and somebody about to fix this wants all of them.
test_every_overlapping_path_is_named if {
	d := derived.decision with input as {
		"repo": {"private_paths": [
			{"path": ".yidam/corpus/a", "holds_content": true},
			{"path": ".yidam/catalog/b", "holds_content": true},
		]},
		"subject": {"kind": "bundle", "sources": sources},
	}
	count(d.deny) == 2
}
