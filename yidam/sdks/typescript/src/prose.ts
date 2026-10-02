/**
 * What a node says, once its class has declared which keys say it.
 *
 * `description` was the only key anything read, and corpora write more than one — `summary`,
 * `findings`, `revisions`, `unfilled`. Reading one field measured 21 lines of a node that says
 * 118 (#674), and across the population 21.8% of a node's bytes against 50.5% (#713). So which
 * keys carry prose is declared, per class, and this module reads a node against a declaration
 * somebody else resolved.
 *
 * **The resolution is not here, and that is the division.** `<class>.ont.yml` names a class's
 * own prose keys, `universal.yml` names the apparatus every class may carry, and the effective
 * set is their union with `description` always in it. Walking those files is a question about a
 * corpus on disk — a directory layout, a filename convention, a git revision — and an SDK that
 * answered it would be answering a question the ontology owns. What is here is the half that
 * has to be identical in three languages, and that `composeEmbedText` needs too.
 *
 * **Two axes, because they are read out of different places.** `keys` are top-level keys, read
 * off the document; `properties` are names under `properties:` the class flagged `prose: true`.
 * Nothing forbids a corpus writing a top-level `method` and a property `method`, so the
 * property keys come back qualified — `properties.method` — and a caller rendering a name can
 * say which one it means.
 *
 * See the Rust reference's `prose.rs` for the full argument.
 */
import type { CorpusInstance } from './corpus.ts'

/**
 * The prose key every node carries whatever anything declares.
 *
 * It is read off `description` rather than out of `extra`, which is the one asymmetry in
 * `proseOf`: `description` is a named field and every other declared key is a coined one.
 */
export const ALWAYS = 'description'

/**
 * This node's prose: the declared top-level keys, then the flagged properties.
 *
 * Both lists are taken in the order given. The caller resolved them, and re-sorting here would
 * make what a node says depend on a rule nobody wrote down.
 *
 * A key the node does not carry, or does not carry as text, yields nothing: a declared
 * `findings:` holding a list is a real state and not prose, and an empty entry would be a blank
 * line in whatever is composed downstream.
 */
export function proseOf(
  inst: CorpusInstance,
  keys: string[],
  properties: string[],
): [string, string][] {
  const out: [string, string][] = []

  for (const key of keys) {
    const value = key === ALWAYS ? inst.description : inst.extra[key]
    if (typeof value === 'string' && value.trim() !== '') out.push([key, value])
  }

  for (const name of properties) {
    const value = inst.properties?.[name]
    if (typeof value === 'string' && value.trim() !== '') {
      out.push([`properties.${name}`, value])
    }
  }

  return out
}

/**
 * The node's prose as one block, the declared fields joined in the order `proseOf` returns them.
 *
 * Each value is trimmed at the end only: leading indentation inside a block scalar is the
 * author's and part of what the node says, while a trailing newline is YAML's and not.
 */
export function proseText(
  inst: CorpusInstance,
  keys: string[],
  properties: string[],
): string {
  return proseOf(inst, keys, properties)
    .map(([, v]) => v.trimEnd())
    .join('\n')
}
