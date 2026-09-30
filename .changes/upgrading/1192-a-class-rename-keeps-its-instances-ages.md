### A class rename keeps its instances' ages

**`yidam migrate class` writes `moved-from:` into every instance it moves (#1192).** Before, a
class rename restarted both commit counts for each instance. Every orphan's escalation clock
reset. Every open question dropped off `due`'s overdue list.

The line is the one `yidam rename` writes, for example `moved-from: ../gage/canyon-outlet.yml`.
An instance that already declared one has it replaced. The migrate plan lists each line it adds.
The JSON report gains a `moved_from` list. Class renames made before this release are not
backfilled.
