/**
 * Stage the reports fixture as a real repository, for running the extension by hand.
 *
 * The extension activates on `workspaceContains:.yidam.toml` or `.yidam/**`, and this
 * repository is not a derived repository — it has no `.yidam/`, so launching against the
 * repo root activates nothing. This produces something to launch against.
 *
 * It reuses the test harness rather than restating it: `stage.toml` is the one copy of how
 * this fixture becomes a repository, and a dev script with its own idea of that would be the
 * eighth transcription of a recipe that until recently had seven.
 */

import * as path from 'node:path'

import { stageInto } from '../test/stage.ts'

const HERE = path.dirname(new URL(import.meta.url).pathname)
/** `<repo>/.local/ext-fixture` — beside the repo-local binary, and git-ignored with it. */
const DEFAULT = path.resolve(HERE, '../../../../.local/ext-fixture')

const target = process.argv[2] ? path.resolve(process.argv[2]) : DEFAULT
stageInto(target)
console.log(target)
