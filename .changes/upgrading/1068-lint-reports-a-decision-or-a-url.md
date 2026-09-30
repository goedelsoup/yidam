### Lint reports a decision or a URL nothing registers

**New Info finding, `decision-uncited` (#1068).** It reports a decision record nothing in the
repository refers to. A markdown link to the record counts. So does its path, `decisions/<stem>`,
named in any file git tracks, code included. A `links:` target or an edge `source:` naming its
`id:` counts. So does a `decision/<id>` entry in `references:`, or another record's `supersedes:`.
Text inside a REGEN block, or in a vendored or generated region, does not count. Across 13
derived corpora, it reported 29 of 346 records. Info, not a gate: your exit code does not change.

**New Info finding, `source-unregistered` (#1068).** It reports a URL in a node or a catalog
body that no catalog `location:` covers. A location covers its own path and every path beneath
it on that host. A corpus that declares no `url` or `url_template` location is not read. To
clear a finding, register the source, or add the URL as a `location:` on its entry. Info, not a
gate: your exit code does not change.
