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
  /**
   * Whether every instance of the class must carry this property.
   *
   * **Absent means false**, and not out of timidity: every corpus written before this field
   * existed was written under a schema where the question could not be asked. Defaulting to
   * `true` would require a declaration nobody made, in every derived repository at once. It
   * is what lets `missing-property` gate at all.
   */
  required: boolean
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
      required: p?.required === true,
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
 * Classes the ontology says nothing points at.
 *
 * The same derivation `orphan-in` exempts on, exposed here so a consumer computing a
 * per-class orphan expectation reads the rule rather than re-deriving it.
 *
 * **It takes the whole ontology, and that is the correction.** This was once
 * `isSourceClass(cls)`, reading one class's own edge list for a `direction: in` entry —
 * which reads half the ontology. `B: {target: A, direction: out}` declares that instances of
 * `B` point at instances of `A`; it is the same fact as `A: {direction: in}` stated from the
 * authoring end, and `target` is *"the class at the other end, whichever end authors the
 * link"*. Reading only a class's own list treated its silence about inbound edges as a
 * positive declaration that nothing points at it. Measured upstream: all three classes of
 * the worked example derived as source classes, so `orphan-in` could not fire anywhere in it.
 *
 * Two things it deliberately does not do:
 *
 * - **A class declaring no edges at all is not a source class.** It has said nothing about
 *   its shape, and reading silence as a declaration would exempt every instance in a corpus
 *   whose ontology is not filled in.
 * - **A self-edge does not make a class pointed at.** `reach -downstream-of-> reach` says
 *   instances relate to each other, not that every instance is cited — any acyclic
 *   self-relation has an endpoint that is not.
 */
export function sourceClasses(classes: OntologyClass[]): Set<string> {
  const pointed = new Set<string>()
  for (const cls of classes) {
    for (const e of cls.edges) {
      if (e.target === cls.name) continue
      if (e.direction === 'in') pointed.add(cls.name)
      else if (e.direction === 'out') pointed.add(e.target)
      else {
        // A declaration that does not say which way it runs exempts neither end.
        pointed.add(cls.name)
        pointed.add(e.target)
      }
    }
  }
  return new Set(
    classes.filter((c) => c.edges.length > 0 && !pointed.has(c.name)).map((c) => c.name),
  )
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
 * Two things about strictness. **A declared property is `required` only where the class says
 * `required: true`** — the compiled schema must be no stricter than the gate, and
 * `missing-property` gates on exactly those and warns for the rest, so the same declaration
 * decides both and neither can outrun the other. **`links[].relationship` is left open** — the gate licenses a
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
    // Emitted for exactly the properties declared `required: true`, and omitted entirely
    // when there are none — an empty `required: []` would be a different document for the
    // same meaning, and these schemas are compared byte for byte across three languages.
    const required = cls.properties.filter((p) => p.required).map((p) => p.name)
    properties.properties = {
      type: 'object',
      properties: declared,
      ...(required.length > 0 ? { required } : {}),
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
