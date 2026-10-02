### Source pack transforms, and `readings:` on a catalog artifact

**A source pack's scheme may name a `describe` and an `extract` transform (#1318).**
Each is a `.glu` file under the pack's `transforms/`, run in the calculator arm's closed prelude.
The binary parses the response as JSON, XML or CSV and hands the transform the parsed value.
`yidam source check` admits each transform and runs it over the scheme's fixtures.

**Transforms need the `source-transforms` feature, which is outside the default build.**
It implies `calculators-gluon`, and adds 3 packages and 0.12 MB over it.
A build without it checks the rest of a pack, and says it did not check the transforms.

**`catalog-extract` records a reading under `readings:`, not `text:`.**
Each reading names its digest, its media type, and what took it in `by:`.
An artifact fetched from an identifier whose pack declares `extract` is read by that transform.
A build without `source-transforms` reports that artifact as skipped.

**What changes for you: nothing you must do.**
An entry that holds its reading under `text:` keeps it, and lint and the vault still read it.
A PDF that already has a `text:` reading is not read again.
