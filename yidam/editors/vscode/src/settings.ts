/**
 * Two settings this extension offers to write, and one it refuses to write twice.
 *
 * Both are decided here, against plain objects, because "should we change the user's
 * settings" is exactly the kind of question that should not be answered inside an
 * activation handler nobody can run.
 *
 * No `vscode` import.
 */

/** The vendored prelude. Read-only by convention, and by nothing else. */
export const VENDOR_GLOB = '**/.yidam/.vendor/**'

/**
 * What `files.readonlyInclude` should become, or null when it already says this.
 *
 * Null is the common case and the important one: activation runs on every window, and a
 * guard that rewrote the setting each time would put a workspace-settings change in every
 * session's git status.
 *
 * `.yidam/.vendor/` is documented read-only in `sadhana/root/AGENTS.md` — an edit there
 * "is silently discarded on the next update" — and **nothing enforces it**. The failure is
 * quiet and delayed: the edit works, survives review, and disappears at the next
 * `mise run yidam-vendor-update`.
 */
export function readonlyUpdate(
  current: Record<string, boolean> | undefined,
  want: boolean,
): Record<string, boolean> | null {
  const existing = current ?? {}
  const has = existing[VENDOR_GLOB] === true
  if (want && has) return null
  if (!want && !(VENDOR_GLOB in existing)) return null
  const next = { ...existing }
  if (want) next[VENDOR_GLOB] = true
  // Removed rather than set false, so turning the guard off leaves the setting as it was
  // found rather than leaving our own entry behind saying "not read-only".
  else delete next[VENDOR_GLOB]
  return next
}

/**
 * What `yidam schema --settings` prints: a mapping per settings key.
 *
 * Read as data rather than as a shape, because the CLI is the one that decides which
 * schemas exist and which globs they cover — a typed model here would be a second opinion
 * about it.
 */
export type SchemaSettings = Record<string, Record<string, unknown>>

export interface Delta {
  /** Settings key, e.g. `yaml.schemas`. */
  key: string
  /** The merged value to write — the user's entries plus ours. */
  value: Record<string, unknown>
  /** The entries this would add or correct, for the prompt. */
  changed: string[]
}

/**
 * What is missing or stale, per settings key. Empty when there is nothing to offer.
 *
 * **Merged, never replaced.** A repository's `yaml.schemas` is a place people put their own
 * mappings, and an "apply" that overwrote them would be a worse failure than the manual
 * copy-paste this replaces.
 */
export function schemaDelta(
  desired: SchemaSettings,
  current: (key: string) => Record<string, unknown> | undefined,
): Delta[] {
  const out: Delta[] = []
  for (const [key, wanted] of Object.entries(desired)) {
    const existing = current(key) ?? {}
    const changed = Object.keys(wanted).filter(
      (k) => JSON.stringify(existing[k]) !== JSON.stringify(wanted[k]),
    )
    if (changed.length === 0) continue
    out.push({ key, value: { ...existing, ...wanted }, changed })
  }
  return out
}

/** One inherited mise task, offered in the palette and to `Run Task`. */
export interface TaskSpec {
  /** The mise task name, which is also the command's suffix: `yidam.task.<name>`. */
  name: string
  title: string
}

/**
 * The tasks RFC-0016 names, and no others.
 *
 * Deliberately not "every task in `mise.yidam.toml`": that file carries a dozen REGEN
 * generators which `regen` already runs in one pass, and offering them individually
 * re-creates the two-lists problem `yidam regen` exists to have solved.
 */
export const TASKS: TaskSpec[] = [
  { name: 'regen', title: 'Refresh REGEN blocks' },
  { name: 'graph-check', title: 'Check the graph (orphans, broken links, missing labels)' },
  { name: 'graph-lint', title: 'Run the lint against the baseline ratchet' },
  { name: 'embed', title: 'Extract embedding text' },
  { name: 'index-build', title: 'Build the semantic index' },
]
