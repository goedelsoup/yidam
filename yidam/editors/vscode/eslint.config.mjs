import tseslint from '@typescript-eslint/eslint-plugin'
import tsparser from '@typescript-eslint/parser'

const ts = {
  languageOptions: { parser: tsparser, ecmaVersion: 2022, sourceType: 'module' },
  plugins: { '@typescript-eslint': tseslint },
  rules: {
    '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_' }],
    'no-console': 'error',
    eqeqeq: 'error',
  },
}

export default [
  { ...ts, files: ['src/**/*.ts', 'test/**/*.ts'] },

  // `scripts/` was passed to eslint by `npm run lint` and matched by no block here, so it
  // was parsed with the default JavaScript parser. That went unnoticed because the one
  // script in the directory happened to contain no TypeScript syntax — the first annotated
  // one failed with `Parsing error: Unexpected token :`, which reads as a broken file
  // rather than an unconfigured directory. `tsconfig.test.json` already includes
  // `scripts/` for exactly this reason; this is the same gap in the other tool.
  //
  // `no-console` is off here and only here. These scripts are run by a person or by CI and
  // their output IS the result: a packaging check that cannot say what is wrong with the
  // package has not checked anything.
  { ...ts, files: ['scripts/**/*.ts'], rules: { ...ts.rules, 'no-console': 'off' } },
]
