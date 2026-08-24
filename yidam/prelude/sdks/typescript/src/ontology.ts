/**
 * The class contract, read and published.
 *
 * `.yidam/corpus/<class>.ont.yml` declares what an instance of a class carries and what it
 * may link to. `yidam lint` **enforces** that declaration; this module **publishes** it, as
 * JSON Schema any editor or validator can apply without linking against yidam.
 *
 * The Rust implementation in `sdks/rust/src/ontology.rs` is the reference and carries the
 * full argument for each decision. The short version, because it is the part that is easy
 * to get wrong here: the compiled schema is deliberately **no stricter than the checks**.
 * A consumer that rejected what the gate accepts would fail somebody's build on a file that
 * looked fine everywhere else.
 */
import { parse as parseYaml } from 'yaml'

export interface OntologyProperty {
  name: string
  /** `string`, `text`, `date`, `ref`, `claim` — or a type this corpus coined. */
  type: string
  description: string
}

export interface OntologyEdge {
  relationship: string
  target: string
  /** `out` when instances of this class author the link, `in` when the other side does. */
  direction: string
  description: string
}

export interface OntologyClass {
  name: string
  label: string
  description: string
  properties: OntologyProperty[]
  edges: OntologyEdge[]
}

/**
 * The evidence tokens a `claim` property may hold, in both spellings.
 *
 * Bare is what a typed vocabulary stores; bracketed is what a corpus writes after being
 * told the prose scan needs brackets. Both are accepted by the counter, so both here.
 */
export const CLAIM_TOKENS = [
  'verified',
  'inference',
  'open',
  '[verified]',
  '[inference]',
  '[open]',
] as const

/**
 * Read a class definition. `name` is the fallback when the file does not name itself.
 *
 * A file that does not parse yields a class that declares nothing, which under the silence
 * rule constrains nothing — the same direction `lint` degrades in.
 */
export function parseClass(name: string, content: string): OntologyClass {
  let doc: Record<string, unknown> = {}
  try {
    const parsed = parseYaml(content) as unknown
    if (parsed && typeof parsed === 'object') doc = parsed as Record<string, unknown>
  } catch {
    doc = {}
  }
  const str = (v: unknown): string => (typeof v === 'string' ? v : '')
  const list = (v: unknown): Record<string, unknown>[] =>
    Array.isArray(v) ? v.filter((i): i is Record<string, unknown> => !!i && typeof i === 'object') : []

  const declared = str(doc.class)
  return {
    name: declared !== '' ? declared : name,
    label: str(doc.label),
    description: str(doc.description),
    properties: list(doc.properties).map((p) => ({
      name: str(p.name),
      type: str(p.type),
      description: str(p.description),
    })),
    edges: list(doc.edges).map((e) => ({
      relationship: str(e.relationship),
      target: str(e.target),
      direction: str(e.direction),
      description: str(e.description),
    })),
  }
}

/**
 * A class nothing is meant to point at: it declares edges, and none of them inbound.
 *
 * A class that declares no edges at all is **not** a source class — it has said nothing
 * about its shape.
 */
export function isSourceClass(cls: OntologyClass): boolean {
  return cls.edges.length > 0 && !cls.edges.some((e) => e.direction === 'in')
}

/**
 * Mirrors `lint`'s `property-type` check, including what it declines to check: a type the
 * corpus coined compiles to `true`, valid against anything.
 */
function propertySchema(type: string): unknown {
  switch (type) {
    case 'string':
    case 'text':
    case 'ref':
      return { type: 'string', minLength: 1 }
    // Structural, not a calendar: what it catches is a date field carrying prose.
    case 'date':
      return { type: 'string', pattern: '^[0-9]{4}(-[0-9]{2}(-[0-9]{2})?)?$' }
    // A list is legal here and nowhere else: the counter reads a list of tags as one claim
    // each, so `claim_tag: [open]` unquoted is a one-element list nobody meant to write.
    case 'claim':
      return {
        anyOf: [{ enum: [...CLAIM_TOKENS] }, { type: 'array', items: { enum: [...CLAIM_TOKENS] } }],
      }
    default:
      return true
  }
}

/**
 * Compile a class definition into a JSON Schema for its instances.
 *
 * Two things it deliberately does not constrain. **No declared property is `required`** —
 * `missing-property` reports and does not gate, so demanding them would reject instances
 * the gate accepts. **`links[].relationship` is left open** — the gate licenses a
 * relationship only for edges landing on another instance, and JSON Schema cannot resolve a
 * path, so a constraint here would reject the `instance-of` link every instance carries.
 * The declared relationships are published as `x-yidam-edges` for completion instead.
 */
export function compileClassSchema(cls: OntologyClass): Record<string, unknown> {
  const schema: Record<string, unknown> = {
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    title: `yidam corpus node — ${cls.name}`,
  }
  if (cls.description !== '') schema.description = cls.description
  schema.type = 'object'

  const properties: Record<string, unknown> = { class: { const: cls.name } }

  // Silence is not a contract. A class declaring no properties constrains none, and in
  // particular does not close the bag — which would reject every instance in a corpus
  // whose ontology is not filled in.
  if (cls.properties.length > 0) {
    const declared: Record<string, unknown> = {}
    for (const p of cls.properties) {
      const body = propertySchema(p.type)
      declared[p.name] =
        typeof body === 'object' && body !== null && p.description !== ''
          ? { ...body, description: p.description }
          : body
    }
    properties.properties = {
      type: 'object',
      properties: declared,
      // Closed, matching `undeclared-property`, which gates.
      additionalProperties: false,
    }
  }

  schema.properties = properties
  schema.required = ['class']
  // Permissive at the top level, as the shared node schema is: derived corpora carry their
  // own fields, and closing this rejected 117 nodes of 117 in one repository.
  schema.additionalProperties = true

  if (cls.edges.length > 0) {
    schema['x-yidam-edges'] = cls.edges.map((e) => ({
      relationship: e.relationship,
      target: e.target,
      direction: e.direction,
    }))
  }
  return schema
}
