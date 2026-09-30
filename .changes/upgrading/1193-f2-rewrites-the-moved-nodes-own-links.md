### F2 rewrites the moved node's own links

**An editor rename between classes now re-relativizes the moved node's links (#1193).** Before,
F2 moved the file and rewrote inbound links. It dropped every edit to the moved node itself. A
link written as `./sibling.yml` was left pointing into the wrong class. `yidam rename` was not
affected.

*What to do.* Run `yidam lint` after any cross-class F2 rename made before this release.
`graph-check` reports each link it left dangling.
