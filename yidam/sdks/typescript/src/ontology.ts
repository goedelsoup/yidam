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
  /** `string`, `text`, `date`, `number`, `ref`, `claim`, `quotation`, `list` — or a type this corpus coined. */
  type: string
  description: string
  /**
   * The unit a `number` is written in, or empty for a dimensionless quantity.
   *
   * **A unit is a fact about the column, not the cell** (RFC-0040). Declared once, here, so
   * an ordering within one property can never compare across units. Published as
   * `x-yidam-unit` on the compiled schema — an annotation, not a constraint.
   */
  unit: string
  /**
   * The closed set a `string` may hold, or empty for an unbounded one.
   *
   * **A value set is a fact about the column, declared where the gate can read it**
   * (RFC-0044). Declaring it closes it: `property-type` reports a value outside it, and the
   * compiled schema carries it as `enum` — a constraint, not an annotation. Empty is absent,
   * and the field is honoured on `string` only.
   */
  values: string[]
  /**
   * Whether every instance of the class must carry this property.
   *
   * **Absent means false**, and not out of timidity: every corpus written before this field
   * existed was written under a schema where the question could not be asked. Defaulting to
   * `true` would require a declaration nobody made, in every derived repository at once. It
   * is what lets `missing-property` gate at all.
   */
  required: boolean
  /**
   * The scalar type every item of a `list` has, or empty (RFC-0050). `values:` and `unit:`
   * on the list apply to each item. Carried and ignored off `list`.
   */
  items: string
  /**
   * The fields of the record every item of a `list` is, or empty. Closed the way a class's
   * property bag is; a `claim` field is the entry's standing, not the node's.
   */
  fields: OntologyProperty[]
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
 * The types an `items:` may name, and a record field may have (RFC-0050). `list` is out
 * because no corpus nests one; `quotation` because it already takes a list of itself.
 */
export const LIST_ITEM_TYPES = ['string', 'text', 'ref', 'date', 'number', 'claim'] as const

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

/** A class definition that does not parse as one YAML mapping. */
export class ClassParseError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ClassParseError'
  }
}

/**
 * Read a class definition. `name` is the class: the `<class>.ont.yml` file stem.
 *
 * **The stem names the class, whatever a `class:` field inside says (#1636).** It is the
 * name every CLI route keys by, so a field that disagrees is not read.
 *
 * **A document that does not parse throws** `ClassParseError`: a YAML syntax error, more
 * than one document, or a top level that is not a mapping. An empty document is not an
 * error; it is a class declaring nothing. Every key is read on its own, and one of the wrong
 * shape reads as absent. The Rust SDK is the reference.
 */
export function parseClass(name: string, content: string): OntologyClass {
  let parsed: unknown
  try {
    parsed = parseYaml(content) as unknown
  } catch (e) {
    throw new ClassParseError(e instanceof Error ? e.message : String(e))
  }
  if (parsed !== null && parsed !== undefined && (typeof parsed !== 'object' || Array.isArray(parsed))) {
    throw new ClassParseError('a class definition is a mapping, and this document is not one')
  }
  const doc = (parsed ?? {}) as Record<string, unknown>
  const str = (v: unknown): string => (typeof v === 'string' ? v : '')
  const list = (v: unknown): Record<string, unknown>[] =>
    Array.isArray(v) ? v.filter((i): i is Record<string, unknown> => !!i && typeof i === 'object') : []
  // The strings of a list, and nothing else: a `values:` written as a mapping, or carrying
  // a bare number, declares no set rather than half of one.
  const strings = (v: unknown): string[] =>
    Array.isArray(v) ? v.filter((i): i is string => typeof i === 'string') : []

  const properties = (v: unknown): OntologyProperty[] =>
    list(v).map((p) => ({
      name: str(p.name),
      type: str(p.type),
      description: str(p.description),
      required: p?.required === true,
      unit: str(p.unit),
      values: strings(p.values),
      items: str(p.items),
      fields: properties(p.fields),
    }))

  return {
    name,
    label: str(doc.label),
    description: str(doc.description),
    properties: properties(doc.properties),
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
 * corpus coined compiles to `true`, valid against anything. A `string` declaring `values:`
 * compiles to that set as `enum` (RFC-0044) — a constraint, because the gate refuses a value
 * outside it. Written as declared: no sorting, no trimming.
 */
function propertySchema(p: OntologyProperty): unknown {
  return p.type === 'list' ? listSchema(p) : scalarSchema(p.type, p.values)
}

/** A property's schema with its `description` and `unit` as annotations. */
function annotatedSchema(p: OntologyProperty): unknown {
  let body = propertySchema(p)
  if (typeof body === 'object' && body !== null) {
    // A local of type `object`: the narrowing above does not survive the first reassignment
    // of `body`, which is declared `unknown`, so a second spread of it would not type-check.
    let annotated: object = body
    if (p.description !== '') annotated = { ...annotated, description: p.description }
    if (p.unit !== '') annotated = { ...annotated, 'x-yidam-unit': p.unit }
    body = annotated
  }
  return body
}

const isItemType = (t: string): boolean => (LIST_ITEM_TYPES as readonly string[]).includes(t)

/**
 * A `list` (RFC-0050): a non-empty array, of one scalar type or one closed record. A
 * declaration the gate reports — `items:` and `fields:` together, or either naming a type it
 * refuses — compiles to `true`: the gate constrains nothing under it.
 */
function listSchema(p: OntologyProperty): unknown {
  const schema: Record<string, unknown> = { type: 'array', minItems: 1 }
  if (p.items !== '' && p.fields.length > 0) return true
  if (p.items !== '') {
    if (!isItemType(p.items)) return true
    // One tag per item: the one-or-a-list tolerance is the property's, not an item's.
    schema.items = p.items === 'claim' ? { enum: [...CLAIM_TOKENS] } : scalarSchema(p.items, p.values)
  } else if (p.fields.length > 0) {
    if (p.fields.some((f) => f.type === 'list' || f.type === 'quotation')) return true
    const declared: Record<string, unknown> = {}
    for (const f of p.fields) declared[f.name] = annotatedSchema(f)
    const required = p.fields.filter((f) => f.required).map((f) => f.name)
    schema.items = {
      type: 'object',
      properties: declared,
      ...(required.length > 0 ? { required } : {}),
      // Closed, as the class's own bag is: `property-type` reports a key no field names.
      additionalProperties: false,
    }
  }
  return schema
}

function scalarSchema(type: string, values: string[]): unknown {
  if (type === 'string' && values.length > 0) return { type: 'string', minLength: 1, enum: values }
  switch (type) {
    case 'string':
    case 'text':
    case 'ref':
      return { type: 'string', minLength: 1 }
    // Structural, not a calendar: what it catches is a date field carrying prose.
    case 'date':
      return { type: 'string', pattern: '^[0-9]{4}(-[0-9]{2}(-[0-9]{2})?)?$' }
    // A YAML number, unquoted: `"7"` is text and the gate says so — the mirror of the date
    // arm's advice to quote `00060`. One arm, not `integer` beside it (RFC-0040).
    case 'number':
      return { type: 'number' }
    // A list is legal here and nowhere else: the counter reads a list of tags as one claim
    // each, so `claim_tag: [open]` unquoted is a one-element list nobody meant to write.
    case 'claim':
      return {
        anyOf: [{ enum: [...CLAIM_TOKENS] }, { type: 'array', items: { enum: [...CLAIM_TOKENS] } }],
      }
    // A span and the catalog entry it is quoted from, one or a non-empty list (RFC-0046).
    // Only the shape: whether the words are in the bytes needs the vault cache.
    case 'quotation': {
      const one = {
        type: 'object',
        properties: {
          of: { type: 'string', minLength: 1 },
          span: { type: 'string', minLength: 1 },
          sha256: { type: 'string', pattern: '^[0-9a-f]{64}$' },
        },
        required: ['of', 'span'],
        additionalProperties: false,
      }
      return { anyOf: [one, { type: 'array', minItems: 1, items: one }] }
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
    for (const p of cls.properties) declared[p.name] = annotatedSchema(p)
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
