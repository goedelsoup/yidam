/**
 * Generate the report contract's TypeScript types from `report.schema.json` (#1645).
 *
 * The twin of `yidam/editors/vscode/scripts/gen-reports.ts`, with the same options and the
 * same banner, so the two packages' copies are byte-identical. Why there are two copies and
 * not one: `test/boundary.mjs` refuses an import from `src/` that leaves this package root,
 * and the extension compiles with `rootDir: src`, so neither can reach the other's file.
 *
 * `npm run gen:reports` writes `src/lib/report-schema.generated.d.ts`. `test/report-schema.mjs`
 * regenerates it in memory and fails when the committed copy differs from it or from the
 * extension's, so a schema change that was not regenerated, and a hand edit to either copy,
 * go red in `ci-editor-web`.
 *
 * A `.d.ts`, imported with `import type` only, so nothing of it reaches `dist/`.
 */

import { readFileSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { compile } from 'json-schema-to-typescript'

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')

export const SCHEMA = path.resolve(ROOT, '../../sdks/parity/fixtures/reports/report.schema.json')
export const OUTPUT = path.join(ROOT, 'src/lib/report-schema.generated.d.ts')
/** The extension's copy, which must be byte-identical to this one. */
export const PEER = path.resolve(ROOT, '../vscode/src/report-schema.generated.d.ts')

const BANNER = `/**
 * GENERATED from yidam/sdks/parity/fixtures/reports/report.schema.json. Do not edit.
 *
 * Regenerate with \`npm run gen:reports\` in yidam/editors/vscode AND yidam/editors/web; the
 * two copies are byte-identical, and each package's tests regenerate its copy and fail when
 * it differs from what is committed (#1645).
 */
`

/**
 * The generated text. Deterministic: no timestamp, the schema's own key order, and a fixed
 * prettier style rather than whatever configuration the working directory happens to hold.
 *
 * @returns {Promise<string>}
 */
export async function generate() {
  const schema = JSON.parse(readFileSync(SCHEMA, 'utf8'))
  const body = await compile(schema, 'YidamReportEnvelope', {
    bannerComment: '',
    // See the extension's copy of this script: an index signature on every object is not
    // what "consumers MUST ignore unknown fields" means.
    additionalProperties: false,
    unreachableDefinitions: true,
    format: true,
    style: { printWidth: 100, semi: true, singleQuote: false, trailingComma: 'all' },
    cwd: path.dirname(SCHEMA),
  })
  return BANNER + body
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  writeFileSync(OUTPUT, await generate())
  console.log(`wrote ${path.relative(ROOT, OUTPUT)}`)
}
