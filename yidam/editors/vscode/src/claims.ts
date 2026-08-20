/**
 * Claim tags, as the editor draws them.
 *
 * `docs/aesthetic-direction.md` calls `[verified]` / `[inference]` / `[open]` "first-class
 * visual states". Until now nothing made them so: the design system defines the palette and
 * the corpus is dense with the tokens, and in between there was plain text.
 *
 * # Where the match comes from
 *
 * Wherever `crate::claims::count_in_source` counts — **exact bracketed tokens, anywhere in
 * the file**. Not "inside a `description:` block", which is the narrower reading: the corpus
 * routinely records absence in a property (`estimate: "[open] — not computed"`), and a
 * decoration that skipped those would disagree with `yidam status` about what a claim is.
 * That disagreement is invisible and permanent, which is the worst kind.
 *
 * Exact rather than a loose bracket match, for the same reason the CLI is exact: corpus
 * prose is dense with markdown links, and `[open questions](…)` is not an open claim.
 *
 * # Where the colours come from
 *
 * `yidam/design/tokens/colors.css`, transcribed. A test parses that file and fails when
 * these drift from it, so the copy is checked rather than trusted.
 *
 * No `vscode` import.
 */

/** The tokens, matching `yidam/cli/src/claims.rs`. */
export const TAGS = ['verified', 'inference', 'open'] as const
export type Tag = (typeof TAGS)[number]

export interface Palette {
  bg: string
  fg: string
  border: string
}

/** Transcribed from `yidam/design/tokens/colors.css`, and checked against it by test. */
export const LIGHT: Record<Tag, Palette> = {
  verified: { bg: '#eaf5ec', fg: '#1a5230', border: '#a8d8b2' },
  inference: { bg: '#fdf4e3', fg: '#764800', border: '#f0d090' },
  open: { bg: '#e6eef9', fg: '#173670', border: '#9dbde8' },
}

export const DARK: Record<Tag, Palette> = {
  verified: { bg: '#16261a', fg: '#a8d8b2', border: '#2f5a3c' },
  inference: { bg: '#2a2113', fg: '#f0d090', border: '#5c4517' },
  open: { bg: '#131d2c', fg: '#9dbde8', border: '#2c4368' },
}

export interface Hit {
  tag: Tag
  /** 0-based. */
  line: number
  start: number
  end: number
}

/**
 * Every claim token in a document.
 *
 * Plain `indexOf` scanning rather than a regex: the tokens are literals, and a regex with
 * an alternation over user-facing text is one escape bug away from matching something else.
 */
export function findClaims(text: string): Hit[] {
  const hits: Hit[] = []
  const lines = text.split('\n')
  for (let line = 0; line < lines.length; line += 1) {
    for (const tag of TAGS) {
      const token = `[${tag}]`
      let from = 0
      for (;;) {
        const at = lines[line].indexOf(token, from)
        if (at === -1) break
        hits.push({ tag, line, start: at, end: at + token.length })
        from = at + token.length
      }
    }
  }
  return hits
}

/**
 * Whether to decorate at all, given the theme kind.
 *
 * Off in high-contrast, per RFC-0016. A high-contrast theme is a stated accessibility
 * requirement, and tinting text against it is the extension overriding a choice the reader
 * made deliberately.
 *
 * `kind` takes VS Code's numbering: 1 Light, 2 Dark, 3 HighContrast, 4 HighContrastLight.
 */
export function shouldDecorate(kind: number, enabled: boolean): boolean {
  return enabled && kind !== 3 && kind !== 4
}
