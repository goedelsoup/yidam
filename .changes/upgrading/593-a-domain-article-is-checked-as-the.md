### A domain article is checked, as the genesis commit holds it

**Four new checks read `.yidam/constitution/` (#593, RFC-0047).** Bootstrap now writes a
constitutional augmentation there, with its Rego rule and cases. `yidam lint` reads each rule
from the genesis commit. `domain-article-violated` fails on a rule's refusal.
`domain-article-edited` fails when a file there differs from the genesis commit.
`domain-article-unproven` warns on a rule with no cases, or a failing case.
`domain-article-unverifiable` warns in a shallow clone, where no rule is read.

**Nothing changes without the directory.** No derived repository has one yet. An article appended
to the vendored `CONSTITUTION.md` did not survive `yidam-vendor-update`, and none was found.
