/**
 * Generate the report contract's TypeScript types from `report.schema.json` (#1645).
 *
 * The schema is the contract. Before this, the editors' types for it were written by hand
 * and trusted by a cast at the process boundary, so a field the CLI renamed or dropped was
 * `undefined` at the point of use rather than an error anywhere. Now `src/reports.ts` takes
 * every field's type from the generated file, and a field the schema loses is a compile
 * error in the alias that names it.
 *
 * `npm run gen:reports` writes the file. `test/report-schema.test.ts` regenerates it in
 * memory and fails when the committed copy differs, so a schema change that was not
 * regenerated, and a hand edit to the output, both go red in `ci-vscode`.
 *
 * The web editor carries a byte-identical copy at `src/lib/report-schema.generated.d.ts`,
 * written by its own `gen:reports` with the same options. One shared path cannot work: this
 * package compiles with `rootDir: src`, and the web editor's boundary gate refuses an import
 * that leaves its package root. Each package's test regenerates its own copy and compares it
 * with the other's.
 *
 * A `.d.ts`, so `tsc` emits nothing for it and nothing reaches `out/` or the `.vsix`: every
 * importer uses `import type`, which is erased.
 */

import { readFileSync, writeFileSync } from 'node:fs'
import * as path from 'node:path'
import { fileURLToPath } from 'node:url'

import { compile } from 'json-schema-to-typescript'

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')

export const SCHEMA = path.resolve(ROOT, '../../sdks/parity/fixtures/reports/report.schema.json')
export const OUTPUT = path.join(ROOT, 'src/report-schema.generated.d.ts')
/** The web editor's copy, which must be byte-identical to this one. */
export const PEER = path.resolve(ROOT, '../web/src/lib/report-schema.generated.d.ts')

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
 */
export async function generate(): Promise<string> {
  const schema = JSON.parse(readFileSync(SCHEMA, 'utf8'))
  const body = await compile(schema, 'YidamReportEnvelope', {
    bannerComment: '',
    // The schema leaves `additionalProperties` unset almost everywhere, which the generator
    // would read as an index signature on every object. Consumers MUST ignore unknown
    // fields, which is a rule about reading, not a field of type `unknown` on every shape.
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
