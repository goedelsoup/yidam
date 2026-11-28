/**
 * The report types, held to `report.schema.json` and to the goldens (#1645).
 *
 * The web half of `yidam/editors/vscode/test/report-schema.test.ts`, and the reason the shared
 * file cannot drift from either end: this package regenerates its own copy and compares it
 * with what is committed, and with the extension's copy, so a schema change regenerated in one
 * package only fails here as well as there.
 *
 * Then the two views this page reads, `LintReport` and `GraphReport`, against the goldens they
 * describe — by the type checker, in-process, over a virtual source file per golden. What none
 * of this checks is a live binary's stdout: `src/lib/cli.ts` parses and trusts it.
 */

import { strict as assert } from 'node:assert'
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs'
import path from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

import ts from 'typescript'

import { generate, OUTPUT, PEER, SCHEMA } from '../scripts/gen-reports.mjs'

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const FIXTURES = path.dirname(SCHEMA)

test('the committed report types are what gen:reports writes from report.schema.json', async () => {
  assert.ok(
    readFileSync(OUTPUT, 'utf8') === (await generate()),
    `${path.relative(ROOT, OUTPUT)} differs from report.schema.json's output: run ` +
      '`npm run gen:reports` here and in yidam/editors/vscode, and commit both copies',
  )
})

test("the extension's copy of the report types is byte-identical to this one", () => {
  assert.ok(
    readFileSync(OUTPUT, 'utf8') === readFileSync(PEER, 'utf8'),
    `${path.relative(ROOT, PEER)} differs from ${path.relative(ROOT, OUTPUT)}: run ` +
      '`npm run gen:reports` in both packages',
  )
})

/** The view each golden is read as on this page, by golden name. */
const VIEWS = {
  lint: { from: '../src/lib/lint.ts', type: 'LintReport' },
  graph: { from: '../src/lib/graph.ts', type: 'GraphReport' },
}

test('the lint and graph goldens are values of the views this page reads them as', () => {
  /** @type {Map<string, string>} */
  const virtual = new Map()
  for (const fixture of readdirSync(FIXTURES).sort()) {
    const expected = path.join(FIXTURES, fixture, 'expected')
    if (!statSync(path.join(FIXTURES, fixture)).isDirectory() || !existsSync(expected)) continue
    for (const [name, view] of Object.entries(VIEWS)) {
      const file = path.join(expected, `${name}.json`)
      if (!existsSync(file)) continue
      virtual.set(
        path.join(ROOT, 'test', `golden.${fixture}.${name}.ts`),
        [
          "import type { YidamReportEnvelope } from '../src/lib/report-schema.generated.d.ts'",
          `import type { ${view.type} as View } from '${view.from}'`,
          // `satisfies` for the contextual type, which keeps `"error"` a literal rather than
          // widening it to `string`; then a variable, not the literal, because the view is a
          // subset of the fields by design and is asked only whether the golden's types fit.
          `const golden = ${readFileSync(file, 'utf8').trim()} satisfies YidamReportEnvelope`,
          'export const view: View = golden',
          '',
        ].join('\n'),
      )
    }
  }
  // An empty glob would pass the assertion below.
  assert.equal(virtual.size, Object.keys(VIEWS).length, 'expected one lint and one graph golden')

  const config = ts.getParsedCommandLineOfConfigFile(path.join(ROOT, 'tsconfig.json'), {}, {
    ...ts.sys,
    onUnRecoverableConfigFileDiagnostic: (d) => assert.fail(String(d.messageText)),
  })
  assert.ok(config, 'tsconfig.json did not parse')
  const options = { ...config.options, noEmit: true }

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
  assert.equal(diagnostics.length, 0, `goldens disagree with this page's report types:\n${text}`)
})
