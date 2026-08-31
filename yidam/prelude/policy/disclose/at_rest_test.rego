# Cases for the weakest of the three, and the reason it is the weakest.
package yidam.disclose.at_rest_test

import data.yidam.disclose.at_rest

declared := {"private_paths": [{"path": "dossier", "holds_content": true}]}

test_public_repository_refuses_declared_material if {
	d := at_rest.decision with input as {"repo": object.union(declared, {"is_private": false})}
	not d.allow
}

# Being private makes it acceptable *here*. That is the right rule for a checkout
# and the wrong one for anything that leaves, which is why `derived` and `record`
# do not consult it — the artifact outlives the access.
test_private_repository_permits_declared_material if {
	d := at_rest.decision with input as {"repo": object.union(declared, {"is_private": true})}
	d.allow
}

test_a_placeholder_directory_is_not_material if {
	d := at_rest.decision with input as {"repo": {
		"is_private": false,
		"private_paths": [{"path": "dossier", "holds_content": false}],
	}}
	d.allow
}
