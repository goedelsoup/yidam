/**
 * One canonical embed text, and the reason there has to be exactly one.
 *
 * An index is built out of a string per node, and *which* string is a judgement: a label, the
 * prose the class declared, the identifiers a query is actually typed in, the names of the
 * things this node points at. Two implementations of that judgement over one corpus produce two
 * different vector spaces from the same weights, and nothing about either looks wrong.
 *
 * That is not hypothetical. RFC-0007 was filed about two assemblers over one corpus — this
 * repository's, composing `label · description · Related: <stems>.`, and a downstream consumer's,
 * composing `label · description · class · <curated meta>` because the SDK shipped no opinion and
 * it had to invent one. Each was reasonable. Neither could be compared to the other, because
 * neither was written down anywhere both could read.
 *
 * So the assembler is a parity function, held to a shared fixture, and `composeEmbedText` is it.
 * See the Rust reference's `embed.rs` for the full argument: the three field lists and who
 * decides them, why `retrievable` is a third axis and not prose, why the union runs one way, and
 * why absent means false.
 */
import type { CorpusInstance, CorpusLink } from './corpus.ts'
import { proseOf } from './prose.ts'

/**
 * What one class declared about which of its fields belong in an embedding.
 *
 * A named shape rather than three positional arrays, because the three lists are not
 * interchangeable and a caller passing them in the wrong order would type-check. It is also what
 * makes the *union* impossible to forget — the mistake `embed` made for as long as it asked
 * `prose` alone, and missed every flagged identifier in the corpus.
 */
export interface EmbedFields {
  /** Top-level keys that carry prose, in the order the caller resolved them. */
  prose_keys: string[]
  /** Property names the class flagged `prose: true`, in declaration order. */
  prose_properties: string[]
  /** Property names the class flagged `retrievable: true`, in declaration order. */
  retrievable_properties: string[]
}

/**
 * Everything this node says that belongs in its embedding: its prose, then the properties flagged
 * `retrievable` that prose did not already carry.
 *
 * Keys come back qualified the way `proseOf` qualifies them — `properties.units`. That
 * qualification is also what makes the de-duplication exact rather than a guess: a property
 * flagged both `prose` and `retrievable` arrives under one key from both sides and is emitted
 * once, while a top-level `parameter` and a property `parameter` stay two things.
 */
export function embedOf(
  inst: CorpusInstance,
  fields: EmbedFields,
): [string, string][] {
  const out = proseOf(inst, fields.prose_keys, fields.prose_properties)
  const already = new Set(out.map(([k]) => k))

  for (const name of fields.retrievable_properties) {
    const key = `properties.${name}`
    if (already.has(key)) continue
    const value = inst.properties?.[name]
    if (typeof value === 'string' && value.trim() !== '') out.push([key, value])
  }

  return out
}

/**
 * The file stem of a link target, with hyphens read as spaces.
 *
 * **Written out rather than taken from the platform's path type**, and that is the fix rather
 * than a style preference. The Rust reference reached this through `Path::file_stem`, which
 * splits on `\` as well as `/` when the target happens to be compiled for Windows — so the same
 * corpus composed two different strings depending on where the binary was built, and no fixture
 * could have said which was right. The rule is four lines and belongs where all three languages
 * can read it:
 *
 * 1. the segment after the last `/`, ignoring a trailing one;
 * 2. never `.` or `..`, which name a directory and not a node;
 * 3. cut at the last `.` that is not the first character, so `plan.v2.yml` is `plan.v2` and a
 *    dotfile keeps its name;
 * 4. `-` reads as a space, because `lower-canyon` is two words to an embedder and one token to
 *    nobody.
 */
function linkStem(target: string): string | null {
  const trimmed = target.replace(/\/+$/u, '')
  const name = trimmed.slice(trimmed.lastIndexOf('/') + 1)
  if (name === '' || name === '.' || name === '..') return null

  const dot = name.lastIndexOf('.')
  const stem = dot > 0 ? name.slice(0, dot) : name
  return stem.replaceAll('-', ' ')
}

/**
 * The names of the things this node points at, in the order it wrote them.
 *
 * A class definition is not one of them. `.ont.yml` is the contract an instance is written
 * *under*, not a thing it relates to, and every instance of a class carries the same one — so
 * admitting it would add one constant phrase to every vector in the class and distinguish
 * nothing.
 */
function related(links: CorpusLink[]): string[] {
  const out: string[] = []
  for (const link of links) {
    if (link.target === null || link.target.endsWith('.ont.yml')) continue
    const stem = linkStem(link.target)
    if (stem !== null) out.push(stem)
  }
  return out
}

/**
 * Compose the text this node is embedded as.
 *
 * Three parts, joined by a single space, each omitted when it is empty:
 *
 * 1. the node's `label`;
 * 2. everything `embedOf` returns, each value trimmed at the end and joined by newlines;
 * 3. `Related: <names>.` over the link targets.
 *
 * **The class is not in the text, and that is a decision rather than an omission.** It is a
 * column beside the vector, where a filter can use it exactly; folded into the text it would put
 * one identical phrase in every vector of a class, which pulls the whole class toward one point
 * and distinguishes no member of it from any other. The consumer RFC-0007 was filed about folded
 * it in, which is one of the two places the two assemblers disagreed.
 */
export function composeEmbedText(
  inst: CorpusInstance,
  fields: EmbedFields,
): string {
  const parts: string[] = []

  if (inst.label !== null && inst.label !== '') parts.push(inst.label)

  const said = embedOf(inst, fields)
    .map(([, v]) => v.trimEnd())
    .join('\n')
  if (said !== '') parts.push(said)

  const names = related(inst.links ?? [])
  if (names.length > 0) parts.push(`Related: ${names.join(', ')}.`)

  return parts.join(' ')
}
