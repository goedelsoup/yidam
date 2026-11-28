/**
 * The report types, held to `report.schema.json` and to the goldens (#1645).
 *
 * Three things, each of which used to be a matter of trust:
 *
 * - `src/report-schema.generated.d.ts` is what `npm run gen:reports` writes from the schema
 *   today. A schema change nobody regenerated, and a hand edit to the output, both fail here.
 * - The web editor's copy is byte-identical to this one. Its own test regenerates it; this one
 *   says the two packages agree, so neither can be regenerated alone.
 * - Every committed golden under `sdks/parity/fixtures/reports/<fixture>/expected/` is a
 *   value of the generated envelope, and each golden this extension reads is a value of the
 *   view it is read as. The type checker decides that, in-process, over a virtual source file
 *   per golden: the golden as an object literal `satisfies YidamReportEnvelope`, which brings
 *   the excess-property check with it, so a renamed field fails as well as a retyped one.
 *
 * What none of this checks is a live binary's stdout: `fetchReport` still casts, and the
 * handshake's `format_version` is the only thing standing between a different contract and
 * these types at runtime.
 */

import assert from 'node:assert/strict'
import * as fs from 'node:fs'
import * as path from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

import ts from 'typescript'

import { generate, OUTPUT, PEER, SCHEMA } from '../scripts/gen-reports.ts'

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const FIXTURES = path.dirname(SCHEMA)

test('the committed report types are what gen:reports writes from report.schema.json', async () => {
  const committed = fs.readFileSync(OUTPUT, 'utf8')
  assert.ok(
    committed === (await generate()),
    `${path.relative(ROOT, OUTPUT)} differs from report.schema.json's output: run ` +
      '`npm run gen:reports` here and in yidam/editors/web, and commit both copies',
  )
})

test("the web editor's copy of the report types is byte-identical to this one", () => {
  assert.ok(
    fs.readFileSync(OUTPUT, 'utf8') === fs.readFileSync(PEER, 'utf8'),
    `${path.relative(ROOT, PEER)} differs from ${path.relative(ROOT, OUTPUT)}: run ` +
      '`npm run gen:reports` in both packages',
  )
})

/**
 * The view each golden is read as, by golden name. A golden absent here is held to the
 * envelope alone; a view absent here (`doctor`, `regen --check`) has no golden to hold it to.
 */
const VIEWS: Record<string, { from: string; type: string }> = {
  lint: { from: '../src/reports.ts', type: 'LintReport' },
  'graph-check': { from: '../src/reports.ts', type: 'GraphCheckReport' },
  status: { from: '../src/reports.ts', type: 'StatusReport' },
  'corpus-index': { from: '../src/reports.ts', type: 'CorpusIndexReport' },
  'open-questions': { from: '../src/reports.ts', type: 'OpenQuestionsReport' },
  phases: { from: '../src/reports.ts', type: 'PhasesReport' },
  'index-status': { from: '../src/reports.ts', type: 'IndexStatusReport' },
  sangha: { from: '../src/reports.ts', type: 'SanghaReport' },
  'catalog-audit': { from: '../src/reports.ts', type: 'CatalogAuditReport' },
  graph: { from: '../src/graph.ts', type: 'GraphReport' },
  neighbors: { from: '../src/neighborhood.ts', type: 'NeighborsReport' },
  vocabulary: { from: '../src/vocabulary.ts', type: 'VocabularyReport' },
  'vocabulary-check': { from: '../src/vocabulary.ts', type: 'VocabularyReport' },
}

/** Every `<fixture>/expected/<name>.json`, as `[fixture, name, absolute path]`. */
function goldens(): [string, string, string][] {
  const found: [string, string, string][] = []
  for (const fixture of fs.readdirSync(FIXTURES).sort()) {
    const expected = path.join(FIXTURES, fixture, 'expected')
    if (!fs.statSync(path.join(FIXTURES, fixture)).isDirectory() || !fs.existsSync(expected)) {
      continue
    }
    for (const file of fs.readdirSync(expected).sort()) {
      if (file.endsWith('.json')) {
        found.push([fixture, file.slice(0, -'.json'.length), path.join(expected, file)])
      }
    }
  }
  return found
}

/** The virtual source for one golden: the literal against the envelope, then its view. */
function source(name: string, json: string): string {
  const view = VIEWS[name]
  const lines = [
    "import type { YidamReportEnvelope } from '../src/report-schema.generated.d.ts'",
    ...(view ? [`import type { ${view.type} as View } from '${view.from}'`] : []),
    `export const golden = ${json.trim()} satisfies YidamReportEnvelope`,
    // A variable, not the literal: the view is a subset of the fields by design, so it is
    // asked only whether the golden's types fit it, not whether the golden has more.
    ...(view ? ['export const view: View = golden'] : []),
  ]
  return lines.join('\n') + '\n'
}

test('every golden is a value of the generated envelope, and of the view the editor reads it as', () => {
  const all = goldens()
  // An empty glob would pass every assertion below.
  assert.ok(all.length > 0, `no goldens found under ${FIXTURES}`)
  for (const name of Object.keys(VIEWS)) {
    assert.ok(
      all.some(([, n]) => n === name),
      `VIEWS names a golden that no fixture has: ${name}`,
    )
  }

  const config = ts.getParsedCommandLineOfConfigFile(
    path.join(ROOT, 'tsconfig.test.json'),
    {},
    { ...ts.sys, onUnRecoverableConfigFileDiagnostic: (d) => assert.fail(String(d.messageText)) },
  )
  assert.ok(config, 'tsconfig.test.json did not parse')
  const options = { ...config.options, noEmit: true, noUnusedLocals: false }

  // Each golden is a file under test/ that exists only in memory, so module resolution from
  // it is the resolution a real test file gets.
  const virtual = new Map<string, string>()
  for (const [fixture, name, file] of all) {
    virtual.set(
      path.join(ROOT, 'test', `golden.${fixture}.${name}.ts`),
      source(name, fs.readFileSync(file, 'utf8')),
    )
  }

  const host = ts.createCompilerHost(options)
  const getSourceFile = host.getSourceFile
  host.getSourceFile = (fileName, languageVersion, ...rest) => {
    const text = virtual.get(path.resolve(fileName))
    return text === undefined
      ? getSourceFile.call(host, fileName, languageVersion, ...rest)
      : ts.createSourceFile(fileName, text, languageVersion, true)
  }
  const fileExists = host.fileExists
  host.fileExists = (fileName) => virtual.has(path.resolve(fileName)) || fileExists(fileName)
  const readFile = host.readFile
  host.readFile = (fileName) => virtual.get(path.resolve(fileName)) ?? readFile(fileName)

  const program = ts.createProgram([...virtual.keys()], options, host)
  const diagnostics = [
    ...program.getOptionsDiagnostics(),
    ...program.getGlobalDiagnostics(),
    ...[...virtual.keys()].flatMap((file) => {
      const sf = program.getSourceFile(file)
      assert.ok(sf, `${file} did not reach the program`)
      return [...program.getSyntacticDiagnostics(sf), ...program.getSemanticDiagnostics(sf)]
    }),
  ]
  const text = ts.formatDiagnostics(diagnostics, {
    getCanonicalFileName: (f) => f,
    getCurrentDirectory: () => ROOT,
    getNewLine: () => '\n',
  })
  assert.equal(diagnostics.length, 0, `goldens disagree with the report types:\n${text}`)

  const viewed = all.filter(([, n]) => n in VIEWS).length
  assert.ok(viewed > 0, 'no golden was checked against a view')
})
