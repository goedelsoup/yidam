/**
 * The corpus node, as it is written on disk.
 *
 * This module used to hold a Markdown parser — `parseNode`, `extractClaims`, `extractLinks`
 * — that three SDKs agreed about exactly and no product ever called. See the Rust
 * reference's `corpus.rs` for the full argument; the short version is that a node is a YAML
 * document, RFC-0002 said so, RFC-0013 closed the question, and neither function the close
 * specified was written.
 *
 * Unknown top-level keys are KEPT, in `extra`. Closing the shape rejected 117 nodes of 117
 * in one derived repository; dropping them silently is worse, and is what a struct that
 * names only the declared keys does.
 */
import YAML from 'yaml'

/** One relationship, as the instance wrote it. */
export interface CorpusLink {
  target: string | null
  relationship: string | null
  /** A standing, or a list of them. Carried, not interpreted — see #587. */
  claim_tag: unknown | null
  source: string | null
}

/** One claim resting on a node in an installed dependency (RFC-0019). */
export interface ExternalCitation {
  package: string | null
  node: string | null
  commit: string | null
  tag: string | null
  span: string | null
}

/** A corpus node: `.yidam/corpus/<class>/<name>.yml`. */
export interface CorpusInstance {
  class: string | null
  label: string | null
  description: string | null
  properties: Record<string, unknown> | null
  links: CorpusLink[] | null
  cites: ExternalCitation[] | null
  /** Every other top-level key, kept rather than dropped. */
  extra: Record<string, unknown>
}

const DECLARED = new Set(['class', 'label', 'description', 'properties', 'links', 'cites'])

function str(v: unknown): string | null {
  return typeof v === 'string' ? v : null
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v)
}

export function emptyInstance(): CorpusInstance {
  return {
    class: null,
    label: null,
    description: null,
    properties: null,
    links: null,
    cites: null,
    extra: {},
  }
}

/**
 * Parse one corpus node.
 *
 * No `path` argument: see the Rust reference. Unparseable is the empty instance rather than
 * a throw — one bad file must not take a corpus down.
 *
 * `version: '1.2'` is passed explicitly and is part of the contract rather than a default
 * worth inheriting. YAML 1.1 reads `no` as false and `010` as 8; 1.2 reads both as what they
 * look like. The Python SDK implements 1.1 and is corrected toward this reading for
 * timestamps, which is the case a corpus actually hits.
 */
export function parseInstance(text: string): CorpusInstance {
  let doc: unknown
  try {
    doc = YAML.parse(text, { version: '1.2' })
  } catch {
    return emptyInstance()
  }
  if (!isRecord(doc)) return emptyInstance()

  const links = Array.isArray(doc.links)
    ? doc.links.map((l): CorpusLink => {
        const m = isRecord(l) ? l : {}
        return {
          target: str(m.target),
          relationship: str(m.relationship),
          claim_tag: m.claim_tag === undefined ? null : m.claim_tag,
          source: str(m.source),
        }
      })
    : null

  const cites = Array.isArray(doc.cites)
    ? doc.cites.map((c): ExternalCitation => {
        const m = isRecord(c) ? c : {}
        return {
          package: str(m.package),
          node: str(m.node),
          commit: str(m.commit),
          tag: str(m.tag),
          span: str(m.span),
        }
      })
    : null

  const extra: Record<string, unknown> = {}
  for (const [k, v] of Object.entries(doc)) {
    if (!DECLARED.has(k)) extra[k] = v
  }

  return {
    class: str(doc.class),
    label: str(doc.label),
    description: str(doc.description),
    properties: isRecord(doc.properties) ? doc.properties : null,
    links,
    cites,
    extra,
  }
}

/**
 * This node as one JSON-shaped object, which is the cross-language form of the contract.
 *
 * `extra` is nested here and flattened on disk, deliberately: flattening is how the keys are
 * read, and nesting is how they are compared. A fixture that could not tell a declared key
 * from a coined one would pass while the split it exists to check was broken.
 */
export function instanceToJson(i: CorpusInstance): Record<string, unknown> {
  return {
    class: i.class,
    label: i.label,
    description: i.description,
    properties: i.properties,
    links: i.links,
    cites: i.cites,
    extra: i.extra,
  }
}
