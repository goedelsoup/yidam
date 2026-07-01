import Lake
open Lake DSL

package Yidam where
  name := "Yidam"

-- The Yidam library: type-theoretic corpus and resolution model.
-- Source: core.lean at this directory root.
lean_lib Yidam where
  roots   := #[`Core]
  srcDir  := "."
