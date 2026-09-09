/**
 * RFC-0032's reference grammar: one name for a thing, and one parser that reads it.
 *
 * ```text
 * identifier   yidam://<corpus>/<kind>/<path>[@<rev>][#<property-path>]
 * relative     <kind>/<path>  |  <path>          resolved against the containing corpus
 * kind         node | crate | catalog | skill | decision
 * ```
 *
 * A corpus node had eleven string forms across the yidam repository before this module. Two
 * could say which corpus a node came from, one could say which revision, and none could say
 * both — they were what different surfaces invented independently because there was no first
 * form to reuse.
 *
 * **The parser is total.** RFC-0032 was written on the claim that `name-not-a-slug` made a
 * name's character set an invariant. Measured against the sixteen corpora with tracked nodes,
 * fourteen conform and two do not, and `yidam lint --bless` is the supported way to be one of
 * the two (#777). So this module parses structurally for any input and answers
 * `referenceConforms` separately, rather than refusing what it was handed.
 */

/**
 * What a reference names. The `<kind>` slot of the grammar.
 *
 * A closed set of five. `node` is the default for a relative reference, because every relative
 * form in the population it replaced named a node.
 */
export type Kind = 'node' | 'crate' | 'catalog' | 'skill' | 'decision'

const KINDS: readonly Kind[] = ['node', 'crate', 'catalog', 'skill', 'decision']

/** The kind a segment names, or `null` if it names no kind. */
export function kindFromWord(word: string): Kind | null {
  return (KINDS as readonly string[]).includes(word) ? (word as Kind) : null
}

/**
 * How many segments this kind's `<path>` has.
 *
 * Two for a node — `<class>/<name>` — and one for everything else. Not a tidiness rule: it is
 * what makes the relative form unambiguous, because reading a leading kind word as a kind has
 * to leave exactly that many segments behind. See `parseReference`.
 */
export function pathArity(kind: Kind): number {
  return kind === 'node' ? 2 : 1
}

/**
 * One reference, parsed.
 *
 * `rev` is a pin and not identity — RFC-0032 §4.3. `x` and `x@abc` denote the same node in two
 * states, which is the split `ExternalCitation` already made by holding `node` and `commit` as
 * two fields.
 *
 * `fragment` names a declared property path and **never a claim**. RFC-0008 measured 22 claims
 * entering corpora in resolution commits and 0 matching byte-identically at any participating
 * tip, because a sentence search that ends at `\n` extracts one claim as two different strings
 * in two hard-wrapped nodes. Identity by surface form is identity by line wrapping.
 */
export interface Reference {
  corpus: string | null
  kind: Kind
  /** The `<path>` segments, joined by `/`. `<class>/<name>` for a node. */
  path: string
  rev: string | null
  fragment: string | null
}

/** The `<path>` split into its segments. */
export function segmentsOf(r: Reference): string[] {
  return r.path === '' ? [] : r.path.split('/')
}

/**
 * Whether a segment is a slug: lowercase ASCII words joined by single hyphens.
 *
 * It is what fourteen of the sixteen measured corpora already write, and what two do not: a
 * manufacturing corpus writes `NonConformance` and a water-quality one `DischargePoint`, both
 * taking class names from their domain's vocabulary in its own case. Kebab-case is a convention
 * corpora converge on with maturity, not one they start with (#777).
 *
 * Written as an explicit character test rather than a regex, because `\w` and the `i` flag both
 * admit characters this rule excludes and the three languages would then be agreeing by
 * coincidence.
 */
export function isSlug(s: string): boolean {
  if (s === '' || s.startsWith('-') || s.endsWith('-') || s.includes('--')) return false
  for (const c of s) {
    const ok = (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c === '-'
    if (!ok) return false
  }
  return true
}

/**
 * Whether every segment of a reference is a slug, so it needs no escaping in any rendering.
 *
 * The separate predicate RFC-0032 §4.6 calls for. A caller that must emit a URI acts on the
 * answer; the parser does not refuse on its behalf. This is the test `export_rdf.rs` already
 * applies to a foreign alignment IRI — check whether it is dereferenceable and demote it when it
 * is not — turned inward onto our own identifiers, which §2 of that RFC complains it never was.
 */
export function referenceConforms(r: Reference): boolean {
  if (r.corpus !== null && !isSlug(r.corpus)) return false
  if (r.path === '') return false
  const segs = segmentsOf(r)
  if (!segs.every(isSlug)) return false
  if (segs.length !== pathArity(r.kind)) return false
  if (r.rev !== null && !isSlug(r.rev)) return false
  if (r.fragment !== null && (r.fragment === '' || !r.fragment.split('.').every(isSlug))) {
    return false
  }
  return true
}

/**
 * Peel the naming corpus off the front, in whichever of the three ways it can be written.
 *
 * `yidam://<corpus>/…` is the grammar's. `yidam://corpus/…` is RFC-0005's frozen authority,
 * where `corpus` is a collection kind rather than a corpus name — the defect RFC-0032 §1 is
 * about — and it means *this* corpus, so it maps to `null`. `pkg::class/name` is `qualified_id`'s
 * form, the only string in the eleven that could say which corpus.
 */
function splitAuthority(body: string): [string | null, string] {
  if (body.startsWith('yidam://')) {
    const rest = body.slice('yidam://'.length)
    const slash = rest.indexOf('/')
    const authority = slash === -1 ? rest : rest.slice(0, slash)
    const tail = slash === -1 ? '' : rest.slice(slash + 1)
    switch (authority) {
      case 'corpus':
      case '':
        return [null, tail]
      // `skills`/`decisions` spend the authority on the *kind*, so the word moves into the path
      // where the kind split reads it, and the two aliases need no second reader.
      case 'skills':
        return [null, `skill/${tail}`]
      case 'decisions':
        return [null, `decision/${tail}`]
      default:
        return [authority, tail]
    }
  }
  const at = body.indexOf('::')
  if (at > 0 && at + 2 < body.length) {
    return [body.slice(0, at), body.slice(at + 2)]
  }
  return [null, body]
}

/**
 * The node-path spellings the repository already wrote, reduced to `<class>/<name>`.
 *
 * `find_node` tolerated three of these and no contract mentioned any of them. An ad-hoc resolver
 * is what an absent grammar looks like from the inside, so they are admitted here, once, instead
 * of in each surface.
 */
function normaliseNodePath(rest: string): string {
  let out = rest.replace(/^\/+/, '')
  const marker = '.yidam/corpus/'
  const at = out.indexOf(marker)
  if (at !== -1) out = out.slice(at + marker.length)
  if (out.endsWith('/')) out = out.slice(0, -1)
  if (out.endsWith('.yml')) out = out.slice(0, -'.yml'.length)
  return out
}

/** Split the kind off the front of a path, defaulting to `node`. */
function splitKind(rest: string): [Kind, string] | null {
  const cleaned = normaliseNodePath(rest)
  if (cleaned === '') return null
  const count = cleaned.split('/').length
  const slash = cleaned.indexOf('/')
  if (slash !== -1) {
    const head = cleaned.slice(0, slash)
    const kind = kindFromWord(head)
    // The arity test. Reading `head` as a kind must leave exactly that kind's path, or `head`
    // was a class name and the whole string is the path.
    if (kind !== null && count - 1 === pathArity(kind)) {
      return [kind, cleaned.slice(slash + 1)]
    }
  }
  return ['node', cleaned]
}

/**
 * Parse a reference. `null` only when the input names no thing at all.
 *
 * Accepts the canonical `yidam://<corpus>/<kind>/<path>` identifier, the relative forms, and the
 * legacy spellings the repository already wrote — a trailing `.yml`, a full
 * `.yidam/corpus/<class>/<name>.yml` path, `pkg::class/name`, and RFC-0005's frozen resource
 * URIs.
 *
 * **The relative form is unambiguous, by arity.** `<kind>/<path>` and a bare `<path>` share a
 * shape, and a corpus may legitimately declare a class named `node`. The path's segment count
 * decides: `node/concept/foo` is the node `concept/foo`; `node/foo` is the node `foo` in a class
 * called `node`. Where both readings are valid — `skill/foo`, with a class named `skill` — the
 * kind wins, because the grammar puts a kind in that position; and `renderReference` emits the
 * explicit `node/skill/foo` for the other reading, so the round trip is total.
 *
 * **`@` is split from the right and `#` from the left.** A conforming segment contains neither,
 * so the rule is only visible on input that already failed `referenceConforms`.
 */
export function parseReference(input: string): Reference | null {
  const s0 = input.trim()
  if (s0 === '') return null

  const hash = s0.indexOf('#')
  const fragment = hash === -1 ? null : s0.slice(hash + 1)
  const s1 = hash === -1 ? s0 : s0.slice(0, hash)

  const at = s1.lastIndexOf('@')
  const hasRev = at > 0 && at < s1.length - 1
  const rev = hasRev ? s1.slice(at + 1) : null
  const body = hasRev ? s1.slice(0, at) : s1

  const [corpus, rest] = splitAuthority(body)
  const split = splitKind(rest)
  if (split === null) return null
  const [kind, path] = split

  return { corpus, kind, path, rev, fragment }
}

/**
 * Render a reference. The only place an identifier is built.
 *
 * A corpus renders the canonical `yidam://` identifier; its absence renders the relative form.
 * The locator — RFC-0032 §4.2's `https://` rendering, for anything a stranger has to follow — is
 * derived per corpus from a declared base and is not this function's job: an identifier must
 * survive a `.yiz` tarball, a private corpus and an offline clone, none of which have a host.
 *
 * **A relative node reference names its kind when its class would otherwise be read as one.** A
 * node in a class called `skill` renders `node/skill/foo`, not `skill/foo`, so that
 * `parseReference` returns what was rendered. Without that the round trip fails on exactly the
 * corpora that name a class after a kind, and silently.
 */
export function renderReference(r: Reference): string {
  let out = ''
  if (r.corpus !== null) {
    out += `yidam://${r.corpus}/${r.kind}/`
  } else {
    const head = r.path.split('/')[0]
    const shadowsAKind = kindFromWord(head) !== null
    if (r.kind !== 'node' || shadowsAKind) out += `${r.kind}/`
  }
  out += r.path
  if (r.rev !== null) out += `@${r.rev}`
  if (r.fragment !== null) out += `#${r.fragment}`
  return out
}
