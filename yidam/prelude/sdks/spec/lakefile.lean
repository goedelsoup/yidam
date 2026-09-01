import Lake
open Lake DSL

-- `package Yidam` already sets the package name. The `name :=` field that used to sit here
-- passed a `String` where Lake wants a `Name`, so the configuration did not even elaborate:
-- older Lake accepted the string form, which is the tell that this was written against a
-- toolchain nobody recorded. `lean-toolchain` beside this file records it now.
package Yidam

-- The Yidam library: type-theoretic corpus and resolution model.
-- Source: Core.lean at this directory root.
lean_lib Yidam where
  roots   := #[`Core]
  srcDir  := "."
