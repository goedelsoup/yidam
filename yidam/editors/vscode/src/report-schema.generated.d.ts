/**
 * GENERATED from yidam/sdks/parity/fixtures/reports/report.schema.json. Do not edit.
 *
 * Regenerate with `npm run gen:reports` in yidam/editors/vscode AND yidam/editors/web; the
 * two copies are byte-identical, and each package's tests regenerate its copy and fail when
 * it differs from what is committed (#1645).
 */
/**
 * Stable identifier. A consumer keys on this; the prose is free to change. In `lint` it is also half of a violation's identity in the baseline.
 *
 * This interface was referenced by `YidamReportEnvelope`'s JSON-Schema
 * via the `definition` "check_id".
 */
export type CheckId = string;
/**
 * Whether an act is a command to run or a judgement to make (#1442). `tool`: running `argv` from the report's `root` IS the act. `person`: a judgement, named in the prose — a command that only LISTS what to do is not a tool act, which is why a gate's findings are `person`. An agent may draft what a person act asks for; it lands only by a person. A consumer MUST read a missing `kind` as `person`, never as `tool`: a producer older than this field emits none, and the safe reading of silence is the one that stops a loop.
 *
 * This interface was referenced by `YidamReportEnvelope`'s JSON-Schema
 * via the `definition` "act_kind".
 */
export type ActKind = "tool" | "person";
/**
 * How `classify_commit` files a commit, declared once. Total by construction: operational verbs are the explicitly marked case and every other verb is epistemic, which the Dafny spec proves — so this enum is closed and a third member would be a change to that proof, not to a report.
 *
 * This interface was referenced by `YidamReportEnvelope`'s JSON-Schema
 * via the `definition` "commit_kind".
 */
export type CommitKind = "epistemic" | "operational";
/**
 * The lint severity vocabulary, declared once. It had three literal copies — the check's declared level, a violation's effective level, and `vocabulary --subject`'s — and each site keeps its own `description` beside the `$ref` because the three mean different things about the same three words. What is shared is the vocabulary, and a fourth value added to one copy and not the others is the drift this closes.
 *
 * This interface was referenced by `YidamReportEnvelope`'s JSON-Schema
 * via the `definition` "severity".
 */
export type Severity = "error" | "warn" | "info";

/**
 * The machine-readable report contract (RFC-0016 Phase 0). Every `yidam <command> --format json` emits this envelope with the command's own fields flattened alongside. Consumers MUST ignore unknown fields: adding one is not a breaking change, and `format_version` is bumped only when a consumer that understood the previous version would mis-read this one.
 */
export interface YidamReportEnvelope {
  /**
   * Contract major version. A consumer reading an unknown value MUST degrade loudly — disable verdict features and say why — and MUST NOT attempt to parse the rest.
   */
  format_version: "1";
  /**
   * Which yidam produced this. The handshake: a consumer is versioned independently of the binary a repository pins in .yidam.toml, so it must be able to detect skew.
   */
  yidam: {
    /**
     * Crate version.
     */
    version: string;
    /**
     * Short commit of the build, or the literal `unknown` when built without git. Never omitted, so 'not recorded' is distinguishable from 'absent'.
     */
    commit: string;
    /**
     * Cargo features compiled in. `reports` is the light default; others gate whole subcommands, so a consumer can tell 'this binary cannot do that' from 'that failed'.
     */
    features: string[];
  };
  /**
   * Absolute path of the repository the report was computed over.
   */
  root: string;
  /**
   * `lint` only. The verdict — a consumer MUST NOT recompute it.
   */
  gate?: {
    /**
     * Whether the run agrees with the committed baseline. Reflects the gate, not the process exit code: --warn-only exits 0 on a failing gate by design.
     */
    passed: boolean;
    /**
     * Error-severity violations absent from the baseline. These fail CI.
     */
    new_violations: number;
    /**
     * Error-severity violations the baseline already records. Inherited debt; these do not fail CI.
     */
    baselined_violations: number;
    /**
     * Baseline entries with no corresponding violation. These fail CI too — a baseline permitted to be wrong drifts.
     */
    stale_baseline_entries: {
      check: string;
      node: string;
    }[];
    /**
     * Entries that have outlived the expiry the baseline declares, and so no longer forgive the violation they list. These fail CI. Separate from `new_violations` although both fail: an introduced violation is something this change did, an expired one is something the repository agreed to deal with and has not — and a consumer telling somebody they introduced a finding that has sat in their baseline for two hundred commits would be wrong.
     */
    expired_baseline_entries: {
      check: string;
      node: string;
      /**
       * Corpus-touching commits it has stood for.
       */
      commits: number;
    }[];
    /**
     * `lint`: this corpus's `[lint] escalate_after`, the corpus-touching commits a dated finding may hold before it escalates to an error. Absent when `.yidam/config.toml` sets none, which is the state where nothing escalates. Each finding's own `severity` still says whether it escalated; this names the number that decided it (RFC-0028 Erratum 9, #1584).
     */
    escalate_after?: number;
    /**
     * Whether the decision record the baseline names in `expire_record` argues its `expire_after` (RFC-0066 §5.2). Absent when the baseline declares neither. `disagrees` and `not_held` fail the gate; `unargued` is a warning for one minor release.
     */
    expiry_record?: {
      state: "argued" | "unargued" | "disagrees" | "not_held";
      /**
       * The record id the baseline names. Absent when `unargued`.
       */
      record?: string;
      /**
       * The `expire_after` the baseline declares.
       */
      expire_after?: number;
      /**
       * The `expire_after` the record states, when it states one.
       */
      record_states?: number;
    };
  };
  /**
   * `lint` AND `doctor`, and the two emit different item shapes under this one key — which went unstated until #645's recursive gate read it. Each shape is a branch of the `oneOf` below, carrying its own full `required` list, and the two are told apart by exactly that: a `lint` check has no `question` and a `doctor` check has no `title`, so every check validates against one branch and never both. It was declared as one open shape requiring `id` alone between #645 and #658, because the undeclared-field walk in `report_goldens.rs` could not follow a composition keyword — so nothing required a lint check to carry `severity` or a doctor check to carry `verdict`, and `doctor` has no golden to notice. Consumers key on `id` and read the fields their command emits. In both, every check that ran is present including those that found nothing — a check that vanishes when it passes cannot be told from one that never ran.
   */
  checks?: (ALintCheck | ADoctorCheck)[];
  /**
   * `graph-check`, `regen --check`, `index-verify`, `doctor` and `source check`. The gate verdict. A consumer MUST NOT recompute it. On `due` it is true unless `--strict` was passed and a clock is due — being owed is not a failure, and this field is here so a consumer does not infer one. On `cycle` it is true unless `--strict` was passed and something is owed OR blocking.
   */
  passed?: boolean;
  /**
   * `doctor` and `policy test`. For `policy test`, test rules that did not hold against a rule nobody overrode — `changed_by_override` holds the ones a corpus changed deliberately. For `doctor`, checks that are wrong now. The only ones that affect the exit code without `--strict`.
   */
  failed?: number;
  /**
   * `doctor` only. Checks that are actionable and a normal state to be in — a light build with no vector index, a pin from last season. They gate only under `--strict`, because a doctor that goes red on the normal state of a normal install is one people learn to pass `|| true`.
   */
  warned?: number;
  /**
   * `propose` only. The `propose/<head>` ref the proposals are written to. Reported whether or not the run wrote — on `--dry-run` it is where they would have gone.
   */
  branch?: string;
  /**
   * `propose` only. The commit the proposals were drafted against and built on.
   */
  head?: string;
  /**
   * `propose` only. Each drafted commit, in the order it is committed. Three verbs only — `open`, `withdraw`, `close` — because nothing merges itself and nothing synthesizes; see RFC-0020.
   */
  proposals?: {
    /**
     * The epistemic verb the commit subject carries.
     */
    verb: "open" | "withdraw" | "close";
    /**
     * The commit subject, in the closed vocabulary `lint --commits` checks.
     */
    subject: string;
    /**
     * The lint check whose finding this proposal answers.
     */
    check: string;
    /**
     * Repo-relative path of the node or catalog entry the proposal is about.
     */
    node: string;
    /**
     * The finding's own words, which the commit body quotes. Carried so a consumer can check the carriage rule for itself rather than trusting that it held.
     */
    detail: string;
    /**
     * Repo-relative paths the proposed commit changes.
     */
    paths: string[];
  }[];
  /**
   * `propose` only. Findings this run could not draft a proposal for, with the reason. Reported rather than swallowed: a finding silently not proposed about is the exact failure the command exists to remove.
   */
  skipped?: {
    check: string;
    node: string;
    /**
     * Why no proposal could be drafted — a `description:` that is not a block scalar, an entry with no closing frontmatter fence, or a message that would not carry its finding.
     */
    reason: string;
  }[];
  /**
   * `propose` only. What reached the repository. Null on `--dry-run` and on a run with nothing to propose — the two states a consumer must not read as a write.
   */
  written?: {
    branch: string;
    /**
     * Short shas, oldest first — the order `git log --reverse` prints them in.
     */
    commits: string[];
  } | null;
  /**
   * `propose` only. What the corpus licensed via `[propose] withdraw_uncited_after`. Null means no withdrawal may ever be drafted there, which is every corpus that has not said otherwise.
   */
  withdraw_uncited_after?: number | null;
  /**
   * `due` only. The eight clocks, each with the interval this corpus declared for it. A clock in state `undeclared` still reports what it measured: nobody has set it, which is not the same as its reading zero. Nor is it the same as `declined`, which is a corpus that decided against what the clock measures and recorded why — the two read identically until `record` is read.
   */
  clocks?: {
    /**
     * Stable identifier: `index`, `catalog`, `questions`, `phases`, `superseded`, `asks`, `coverage` or `template`.
     */
    id: string;
    /**
     * What the clock measures, in the form a person would ask it.
     */
    question: string;
    /**
     * `due` is not a defect. A corpus with three expired sources is owed, not unhealthy — `doctor` is the report that answers whether anything is wrong. `undeclared` means this corpus set no interval, so the clock can never come due; `declined` means it set none deliberately, having decided against the thing the clock measures, and `record` names where it argued that; `unmeasurable` means it set one and there is nothing to measure against, which is a gap in bookkeeping rather than a stale subject; on `template` alone it may instead mean the origin could not be read, which is a fact about the network the run had. A consumer MUST NOT render `declined` as `undeclared`: the first is a decision and the second is its absence. `unbuildable` is the one state that is a fact about the BINARY rather than about the corpus: the act that would discharge the clock is compiled out of this build, so the clock has no subject here whatever the corpus declared. Only the `index` clock can reach it — `index-build` is behind a feature the released binary does not carry, and every other clock measures something a `reports` build can both read and discharge. It is NOT `due`, and the difference is whether the row can ever change: a clock nothing available to the reader can discharge reads red on every run forever, which is the state two derived repositories met before this one existed (#1061) — one declined the clock to silence it and one wrote a comment into its config explaining the row. Its `remedy` is an install rather than a corpus act, and a consumer MUST NOT count it as owed. A light build holding an index pulled from elsewhere is NOT in this state: reading a `meta.json` needs no feature, so the staleness verdict there is the real one.
     */
    state: "due" | "ok" | "undeclared" | "declined" | "unmeasurable" | "unbuildable";
    /**
     * The declaration that sets this clock's interval, as a reader would write it. Null when this corpus has not set one. The catalog clock reads `[catalog] ttl_days` and each entry's own `ttl_days:`; the other four read `[due]` keys.
     */
    interval?: string | null;
    /**
     * Subjects past the interval. Zero on every state but `due`.
     */
    overdue: number;
    /**
     * What was measured, whether or not an interval applies.
     */
    detail: string;
    /**
     * The command or act that discharges it — or, on an `undeclared` clock, the key that would set it. Null when there is nothing to do, which includes every `declined` clock: nothing discharges a decision not to want the thing.
     */
    remedy?: string | null;
    kind?: ActKind;
    /**
     * The command `remedy` names, as words to execute with the envelope's `root` as the working directory. Present exactly when `kind` is `tool`; a `person` remedy never carries one.
     */
    argv?: string[];
    /**
     * The decision record behind a `declined` clock, repo-relative under `.yidam/decisions/`. Null on every other state. The record is required for a decline to be honoured at all — a `[due.declined]` entry naming a record the repository does not hold leaves the clock in the state it was really in, so a decline cannot rot into a silenced row.
     */
    record?: string | null;
    /**
     * The owed subjects by name, one line each, on a clock whose subjects a person must visit, and only when `due`. On `superseded` each line names a node, the entry it cites and how long ago that entry gained its latest version; on `asks`, an ask and the re-pin it stands against; on `coverage`, a member of a declared universe that no instance carries, and the entry that listed it. Every subject is listed, however many; a text rendering may show fewer. Absent otherwise, which a consumer MUST read as an empty list.
     */
    subjects?: string[];
  }[];
  /**
   * `due` only. Clocks past their interval. A consumer MUST NOT render this as a fault count. `cycle` carries the same count, for the same reason and with the same prohibition.
   */
  due?: number;
  /**
   * `due` only. Clocks this corpus declared no interval for, which therefore can never come due. Declined clocks are NOT counted here — a corpus that decided against something has not left a clock unset.
   */
  undeclared?: number;
  /**
   * `due` and `doctor`. For `due`, clocks this corpus declined in writing under `[due.declined]`, each with a decision record behind it. For `doctor`, checks whose verdict is `declined` under `[doctor.declined]`. Not owed, and not a fault.
   */
  declined?: number;
  /**
   * `due` and `doctor`. Keys under `[due.declined]` naming no clock `due` reads, or under `[doctor.declined]` naming no offer `doctor` makes. Empty in a corpus whose declines all land. A key here is a declaration that reads as deliberate and has no effect, and `doctor --strict` fails on one.
   */
  unknown_declines?: string[];
  /**
   * `doctor` and `due`. Whether `--strict` was passed. It is the only thing that makes a due clock exit nonzero, and the only thing that makes a doctor warning do so. On `cycle` it is what makes being owed or blocked exit nonzero.
   */
  strict?: boolean;
  /**
   * `graph-check` only. Neither instances nor class definitions — a fresh repository, not a failure.
   */
  corpus_empty?: boolean;
  /**
   * `graph-check` only.
   */
  total_instances?: number;
  /**
   * `graph-check` only.
   */
  clean_instances?: number;
  /**
   * `graph-check` only.
   */
  classes_defined?: number;
  /**
   * `graph-check` only.
   */
  nodes_with_issues?: {
    node: string;
    issues: string[];
  }[];
  /**
   * `graph-check` only. Class files whose bytes did not parse, in the same shape as `nodes_with_issues`. Gates: a corpus whose schema cannot be read is not a graph anything has checked.
   */
  classes_with_issues?: {
    node: string;
    issues: string[];
  }[];
  /**
   * `graph-check` only. Reported, never gated.
   */
  classes_without_instances?: string[];
  /**
   * An array in `corpus-index` (index rows), `graph` (nodes with their resolved edges) and `diff` (changed nodes); an integer in `status` (how many there are). Two shapes under one name, which the field-presence check could not see until #650 gave this file a validating reader — the hazard this description named while nothing enforced it.
   */
  nodes?:
    | {
        /**
         * Corpus-relative path. A node's identity throughout, in every report that names one.
         */
        node: string;
        class?: string;
        label?: string;
        /**
         * `graph` only.
         */
        description?: string;
        /**
         * `graph` only. Every edge this node declares, already resolved.
         */
        links?: {
          /**
           * Exactly as written, so a consumer can find it in the file.
           */
          target: string;
          relationship: string;
          /**
           * Corpus-relative path of the resolved target, or empty when it lands outside the corpus. Resolution is `normalize(dir.join(target))` — the rule `dangling-edge` and `orphan-in` apply.
           */
          resolved: string;
          /**
           * Whether that path is a file. This is the gate's own test, and a consumer MUST NOT recompute it: an editor that decided for itself would disagree with `lint` about which edges are broken.
           */
          exists: boolean;
        }[];
        /**
         * `corpus-index` only. How many edges the node declares.
         */
        links_out?: number;
        /**
         * `corpus-index` only.
         */
        claims_verified?: number;
        /**
         * `corpus-index` only.
         */
        claims_inference?: number;
        /**
         * `corpus-index` only.
         */
        claims_open?: number;
        /**
         * `corpus-index` only. The same three over the node's **edges** — its links' `claim_tag` (#857). BESIDE the node counts and never folded into them: a node's claims are measured over its text, an edge is in no node's text and belongs to two nodes at once, so one merged figure would answer about a denominator nobody chose. A consumer wanting the corpus-wide total adds the two. NOT required: a CLI released before this field emits a measurement without it, and its output must still validate.
         */
        edge_claims_verified?: number;
        /**
         * `corpus-index` only. The same three over the node's **edges** — its links' `claim_tag` (#857). BESIDE the node counts and never folded into them: a node's claims are measured over its text, an edge is in no node's text and belongs to two nodes at once, so one merged figure would answer about a denominator nobody chose. A consumer wanting the corpus-wide total adds the two. NOT required: a CLI released before this field emits a measurement without it, and its output must still validate.
         */
        edge_claims_inference?: number;
        /**
         * `corpus-index` only. The same three over the node's **edges** — its links' `claim_tag` (#857). BESIDE the node counts and never folded into them: a node's claims are measured over its text, an edge is in no node's text and belongs to two nodes at once, so one merged figure would answer about a denominator nobody chose. A consumer wanting the corpus-wide total adds the two. NOT required: a CLI released before this field emits a measurement without it, and its output must still validate.
         */
        edge_claims_open?: number;
        /**
         * `corpus-index` only. Length of the node file.
         */
        lines?: number;
      }[]
    | number;
  /**
   * An array in `open-questions` (the nodes carrying one); an integer in `status` (how many there are). Two shapes under one name, the same hazard `nodes` above records — and the same one that went unseen while this contract was checked by field presence alone.
   */
  open_questions?:
    | {
        /**
         * Corpus-relative path.
         */
        node: string;
        /**
         * The node's label, which for a `?`-titled node is the question itself. For a `scope: edge` entry it is still the NODE's label, because the node is the file a reader opens; `relationship` and `target` say which of its edges asked.
         */
        label: string;
        /**
         * Which asked it (#857). An edge tagged `open` is an open question in its own right and is listed as one rather than promoting the node that authors it — a node may author a dozen edges, and an entry saying only that something under it is unsettled sends the reader looking. On EVERY entry, node and edge alike, so the two are told apart by reading a field rather than by noticing that `relationship` is absent. NOT required: a CLI released before this field emits node entries without it, and its output must still validate.
         */
        scope?: "node" | "edge";
        /**
         * `scope: edge` only. The relationship the open edge asserts.
         */
        relationship?: string;
        /**
         * `scope: edge` only. The edge's target, exactly as the node wrote it, so a consumer can find it in the file.
         */
        target?: string;
        /**
         * Present only when EVERY open claim on the entry is written `[open — unanswerable: <reason>]` (#1506, RFC-0061): the first of their reasons. The entry is still an open question and is still listed; what it is off is the questions clock in `due`. Absent rather than null on a plain entry. NOT required: a CLI released before this field emits entries without it, and its output must still validate.
         */
        unanswerable?: "not in the public record" | "out of scope" | "superseded";
        /**
         * Present whenever at least one open claim on the entry is marked unanswerable (#1506, RFC-0061): the reason of every marked claim, declared properties first and then prose, repeats kept. It is present on a fully terminal entry beside `unanswerable`, and on an owed one, where it is the only surface the marked claims appear on. Absent on a plain entry.
         */
        reasons?: ("not in the public record" | "out of scope" | "superseded")[];
      }[]
    | number;
  /**
   * `status` only. How many of `open_questions` are off the questions clock because every open claim on them is marked unanswerable (#1506). A SUBSET of `open_questions`, never added to it. NOT required: a CLI released before this field emits `status` without it, and its output must still validate.
   */
  unanswerable?: number;
  /**
   * `open-questions` only. The questions this repository put to the corpora it depends on, read from `.yidam/asks/<package>/` (#1467): only those with `status: open`, and only those whose header names a pin. BESIDE `open_questions` and never counted in it — whether an ask is settled is decided on the far side, and an undecided claim here is not the same thing. A consumer wanting to know whether one is due a re-read reads the `asks` clock in `due`, which compares `pin` with the dependency's current pin. NOT required: a CLI released before this field emits `open-questions` without it, and its output must still validate.
   */
  asks?: {
    /**
     * Repo-relative path of the ask file.
     */
    ask: string;
    /**
     * The dependency it is addressed to, as `.yidam/tonpa.toml` names it — the ask's directory.
     */
    package: string;
    /**
     * The far-side node the question is about, as the ask's header gives it. Absent for a question to the corpus as a whole.
     */
    node?: string;
    /**
     * The far side's commit the question was read against, as written; it may be abbreviated.
     */
    pin: string;
    /**
     * The ask's first `#` heading, or its file stem when it has none.
     */
    title: string;
  }[];
  /**
   * `catalog-audit` only. One row per catalog entry. Each carries THREE citation counts and they mean different things: `nodes` is corpus instances citing the entry, by a link, an edge `source:`, a quotation's `of:` or an evidenced list entry's `source` — the number every gate reads, and what `catalog-uncited` means by "no corpus node draws on this source"; `elsewhere` is other files under .yidam/corpus/ that link to it, which is class definitions and README prose; `citations` is their sum, retained with exactly the meaning it had before the two were separated. A consumer deciding whether a claim rests on a source MUST read `nodes`, never `citations`. Each row also names WHICH nodes cite it (`cited_by`) and how the entry's own hand-written `used-by` list disagrees with them (`drift`), so a consumer navigating from a node to its sources never re-resolves the corpus's links to find out.
   */
  sources?: {
    /**
     * File name of the entry within .yidam/catalog/.
     */
    entry: string;
    /**
     * The entry's declared `type:`, or an em dash when it declares none.
     */
    type: string;
    description: string;
    /**
     * Absent in the entry means obtained; only an explicit `false` claims otherwise.
     */
    obtained: boolean;
    /**
     * `nodes` + `elsewhere`. Kept so a consumer written against the single-count contract still reads the number it always read.
     */
    citations: number;
    /**
     * Corpus instances linking to this entry. The gate-bearing count.
     */
    nodes: number;
    /**
     * Non-instance corpus files linking to this entry — class definitions and READMEs. Never added to `nodes`: a claim resting on a source and a page linking to one are different things.
     */
    elsewhere: number;
    /**
     * Repo-relative paths of the corpus instances linking to this entry, sorted. Exactly `nodes` of them — the count is this list's length, so the two cannot come to disagree. Instances only: a class definition or a README linking here is counted in `elsewhere` and not named, because it is not a node and has no row in a corpus view to sit under.
     */
    cited_by: string[];
    /**
     * Retained for compatibility (RFC-0053 §2.6). The entry's declared `used-by` list, verbatim and unresolved — which an entry no longer stores, so it is empty on every row once `yidam migrate used-by` has run. `cited_by` is the field to read. The next report contract bump removes it.
     */
    used_by: string[];
    /**
     * Retained for compatibility (RFC-0053 §2.6): always null. It compared a stored `used-by` list to the citations, and with the citations the only record there is nothing to compare. The object shape is kept so a reader written against contract 1 keeps parsing. The next report contract bump removes it.
     */
    drift: {
      /**
       * Basenames the list claims, that no citing node carries.
       */
      claimed_not_citing: string[];
      /**
       * Basenames of nodes that cite this entry, that the list omits.
       */
      citing_not_claimed: string[];
    } | null;
    /**
     * The entry's fetched artifacts, as its record declares them. Empty when the record declares no `artifacts:` key at all — NOT when it names no bytes, a different condition this file stated wrongly until #650, since `catalog.rs` reserves 'names bytes' for an artifact carrying a non-empty `sha256`. A catalog entry is a record about a source and does not have to have fetched one.
     *
     * Items: Every field is optional so that a malformed record parses and is REPORTED rather than making the whole entry unreadable: an entry that fails to load takes its citations, its TTL and its `used-by` down with it, which is a large penalty for one bad field.
     */
    artifacts?: {
      /**
       * The content address — 64 lowercase hex characters.
       */
      sha256?: string | null;
      /**
       * Size in bytes, as fetched.
       */
      bytes?: number | null;
      /**
       * So a reader knows what the artifact is without fetching it.
       */
      media_type?: string | null;
      /**
       * When these bytes were obtained. Distinct from the entry's own `retrieved:`, which is about the record — an entry can be revised without re-fetching anything.
       */
      retrieved?: string | null;
      /**
       * Where it came from: an index into this entry's `location` list, or a literal URL. An untagged enum over the two, so a consumer branches on the JSON type. Declared with a description and no `type` until #650, which under draft 2020-12 is the empty schema and asserts nothing — `yidam schema` has always been stricter about the same field.
       */
      from?: number | string | null;
      /**
       * Which of the resources at `from` these bytes are, when that origin serves more than one — the query an API base URL was asked, the file a download page listed (#1432). Null where the record names none, which reads as the location itself. A new digest is a new version only of a record with the same origin and resource.
       */
      resource?: string | null;
      /**
       * Which vault these bytes may be stored in. `none` means the local cache and nowhere else. Routing, and not permission — `redistributable` is the other question and overrides this one.
       */
      vault?: string | null;
      /**
       * Whether these bytes may leave this machine at all — the licensing bit, which overrides `vault` above. Declared with no `type` until #650, and it is the field where an unasserted contract costs the most.
       */
      redistributable?: boolean | null;
      /**
       * A text reading of these bytes, for an artifact that is not text — a PDF (#1172). Null where none was taken. A quotation of the artifact is compared with the reading, which is itself an artifact in the vault; lint never runs an extractor.
       */
      text?: {
        /**
         * Content address of the reading, 64 lowercase hex.
         */
        sha256?: string | null;
        /**
         * The extractor and version that took the reading, so a person can reproduce it. Never re-run to check one.
         */
        extractor?: string | null;
      } | null;
      /**
       * Derived readings of these bytes (RFC-0048 §6, #1318): `text` generalised. A PDF's text reading is one, a source pack's `extract` transform writes others. Null where none was recorded under this key; an entry written before #1318 records its reading under `text` instead.
       */
      readings?:
        | {
            /**
             * Content address of the reading, 64 lowercase hex.
             */
            sha256?: string | null;
            /**
             * What the reading is: `text/plain` for a text reading, which a quotation is compared with.
             */
            media_type?: string | null;
            /**
             * The file inside a zip this reading is, by its path there, when the location the zip was fetched from names it under `members:` (#1351). Absent for a reading of the whole artifact.
             */
            member?: string | null;
            /**
             * What took the reading: `<crate> <version>` for an extractor, `<pack>@<version>/<transform>@sha256:<hash>` for a source pack's transform, `unzip` for a member read out of a zip. Never re-run to check one.
             */
            by?: string | null;
          }[]
        | null;
    }[];
  }[];
  /**
   * `phases` only. One row per inquiry ref backed by a `ma/*`, `rigpa/*` or `phase/*` branch. Every ref is listed and each says what it is, rather than the table filtering to what it calls active.
   */
  phases?: {
    name: string;
    /**
     * A column rather than a filter: a table listing settled evolutions beside standing positions and calling the lot `active phases` is what this replaced. The enum is closed and is checked against `git::REF_STATES` both ways: a state the code can emit and this does not name is a report a validating consumer rejects, and one named here that the code cannot emit is a branch nobody can take. The only copy, since #660 — an envelope-level `state` stood beside it from 42354da, and no report has ever emitted one. `interrupted` is the one member no ref shape can produce: it means a phase run resolved a plan and its steps did not all complete, which only the phase record knows. Read `source` to learn which evidence decided the value.
     */
    state: "active" | "interrupted" | "settled" | "rewritten" | "position";
    /**
     * `phases` only. The phase type declared at `yidam phase start`, carried by the ref's phase record. ABSENT, never empty or null, where the ref carries no record — which is every phase opened by hand, every phase predating the record, and all 60 `phase/*` refs across the eighteen corpora A0 measured. A consumer MUST NOT supply a default: RFC-0028 Erratum 1 records that a four-element fallback list is the artefact that section exists to prevent.
     */
    type?: string;
    /**
     * `phases` only. Which evidence decided `state`. RFC-0028 §3 ranks two sources rather than collapsing them — `RefKind` answers WHAT IS THIS REF and the phase record answers WHAT HAPPENED IN THIS RUN, and collapsing them recreates #272's defect one level up. `ref` where no record exists, and wherever the answer is about the baseline: `position`, `settled` and `rewritten` are facts a record committed on the phase's own branch cannot hold. `record` only where ref shape says `active` and a record refines it.
     */
    source: "record" | "ref";
    /**
     * The git ref to read, which is not the phase's name when the phase lives only on a remote.
     */
    ref_name: string;
    owner: string;
    started: string;
    commits: number;
  }[];
  /**
   * `diff` only.
   */
  edges?: unknown[];
  /**
   * `diff`, `check-diff`, `log` and `log-epistemic`. The git range reported on, unnormalised, so what was compared is what the report says. Usually exactly as the caller gave it; `check-diff` may be run with no range, and then this is the default it resolved to — the merge-base with the default branch — rather than a blank.
   */
  range?: string;
  /**
   * `check-diff` only. How many names the ontology declares — classes, properties and relationships together. All three, because a domain concept can arrive in the ontology as a property of something else or as the name of a relationship, and a check reading only class names would ask about a concept the corpus already models.
   */
  vocabulary?: number;
  /**
   * `check-diff` only. Type declarations the diff adds, after regions declared `excluded` in .yidam/authorship.yml are dropped. A name the diff also removes is not counted: a struct moved between files is not a new concept.
   */
  introduced?: number;
  /**
   * `check-diff` only. How many of `introduced` match a declared name, kebab-cased. A count and never a finding: one row per correctly implemented concept would be a permanently non-empty report, which is where a real finding gets lost.
   */
  aligned?: number;
  /**
   * `check-diff` only. Types the diff introduces that the ontology does not name. This report never gates: a consumer must not read a non-empty array as a failure. The item shape below was prose here until #650 — a description of a shape is not a declaration of it, and nothing could check the difference.
   */
  findings?: {
    /**
     * Stable id. One check in Phase A, named anyway so a consumer branches on a token rather than on prose.
     */
    check: string;
    /**
     * `info` where the file sits in a declared `generated` or `imported` region, `warn` otherwise. Never `error`: this report does not gate.
     */
    severity: "warn" | "info";
    /**
     * As written in the code: `AgendaItem`.
     */
    concept: string;
    /**
     * What the ontology would have to name for this to match: `agenda-item`.
     */
    name: string;
    /**
     * A declared name this one shares a root with, when the vocabulary holds one. A LEAD AND NEVER A VERDICT: two names sharing a five-character prefix is a fact about two strings, arrived at without a model. A word that ends three or more of this report's names and appears nowhere else in them is not compared, so nine error types are not each offered `margin-of-error`. The finding exists whether or not a candidate was found — so a consumer must not render this as a confidence, a score, or a proposed fix. Absent when nothing shares a root.
     */
    nearest?: {
      /**
       * The declared name, kebab-cased as the vocabulary holds it: `sponsored-by`.
       */
      name: string;
      /**
       * The root the two names have in common: `sponsor`. Evidence, not a score.
       */
      shared: string;
    };
    file: string;
    span: Span1;
    /**
     * Phrased as a question, deliberately — the answer is a person's. Matching is by name, so a type that matches nothing may be a genuine gap in the ontology or a helper the ontology has no reason to know about. The check cannot tell, and must not phrase itself as though it could.
     */
    question: string;
    /**
     * Why a finding here is not this repository's to fix, and whose it is. Present only inside a `generated` or `imported` region.
     */
    region?: string;
  }[];
  /**
   * `lint --bless` only. No gate ran, so no `gate` block is emitted.
   */
  blessed?: {};
  /**
   * `status` and `index-status`. Whether `.yidam/index/` exists. `index-status` also counts the main worktree's index when a linked worktree reads it (RFC-0069); `status` counts only this checkout's.
   */
  index_present?: boolean;
  /**
   * `status --format json` only. Files under `.yidam/computed/` that declare themselves readable — a top-level `format_version` and a `signals:` list. Zero where the directory is absent, which is every corpus that declares no calculator.
   */
  computed_files?: number;
  /**
   * `status --format json` only. Every signal name those files carry, sorted and deduplicated. A name is repository-wide: two files claiming one name is a `doctor` failure, not a namespace.
   */
  computed_signals?: string[];
  /**
   * `status --format json` only. Distinct nodes at least one signal is attached to, keyed in the reference grammar.
   */
  computed_nodes?: number;
  /**
   * `status --format json` only. Computed files no capability's `writes` glob covers, so nothing in the manifest accounts for them.
   */
  computed_unowned?: string[];
  /**
   * `status` only. Bounded work (`rigpa/*`, `phase/*`) not yet an ancestor of the baseline. Counted every `ma/*` and `rigpa/*` ref, and no `phase/*` ref, until the namespaces were typed.
   */
  active_phases?: number;
  /**
   * `status` only. Bounded work already merged whose ref still exists; PHASES.md prescribes deleting these.
   */
  settled_phases?: number;
  /**
   * `status` only. Bounded work whose commits reached the baseline by a merge that rewrote them — GitHub's rebase or squash button — so its ref is not an ancestor of the baseline and never will be. Counted as `active_phases` until #773, where one repository read 22 finished phases as in flight and the number could not fall.
   */
  rewritten_phases?: number;
  /**
   * `replay` only. Corpus health at each commit that touched the corpus, oldest first — the series every other report cannot give, because every other report answers about now. Always the whole series; `--every` samples the text rendering and never this.
   */
  replay?: {
    /**
     * Short hash of the commit this row describes.
     */
    commit: string;
    /**
     * Commit date, `YYYY-MM-DD`. A date and not an age: an age is a function of when you ask.
     */
    date: string;
    /**
     * Instance nodes present at this commit.
     */
    nodes: number;
    /**
     * Nodes nothing points at, excluding classes the ontology says nothing should point at — the same population `orphan-in` reports.
     */
    orphans: number;
    /**
     * Per class, keyed by class name. The corpus-wide figure is the one that misleads: it sums classes nothing is meant to point at with classes whose other instances are cited, and only the second is a finding.
     */
    by_class: {
      [k: string]: {
        nodes: number;
        orphans: number;
        /**
         * Instances of this class that nothing cites.
         */
        uncited: number;
        /**
         * What the ontology declared for this class, reading both ends of every edge: `uncited` for a source class nothing is meant to point at, `cited` for a class the ontology says something points at, `unstated` when it declared neither. Either end may declare the relationship — `direction: in` on one class and `direction: out` on the other state the same fact — so a class carrying no `edges:` of its own can still be `cited`. Being NAMED is not being pointed at, and the difference is the direction: `gauge` declaring `target: concept, direction: in` says instances of a concept point at a gauge, so it says nothing about anything pointing at a concept, and a `concept` declaring nothing stays `unstated` (#659).
         */
        expectation: "cited" | "uncited" | "unstated";
        /**
         * Whether the class is behaving as it declared. `null` when it declared nothing — there is no expectation to meet, and scoring a class against one it never stated is how a corpus with an unfilled ontology gets reported as failing. A consumer MUST NOT read `null` as false.
         */
        meets_expectation: boolean | null;
      };
    };
  }[];
  /**
   * `status` only. `null` in a projection, whose first commit is the host repository's (RFC-0064 §2.3).
   */
  genesis?: string | null;
  /**
   * `status` only. Whether `.yidam/config.toml` declares the corpus written by people or projected by a program (RFC-0064 §1).
   */
  mode?: "authored" | "projection";
  /**
   * `status` only.
   */
  catalog_entries?: number;
  /**
   * `status` only. Source records past the TTL this corpus declared, per entry or under `[catalog]`. Zero when no TTL is declared, which is every corpus that has not opted in.
   */
  catalog_expired?: number;
  /**
   * `status` only.
   */
  claims_verified?: number;
  /**
   * `status` only.
   */
  claims_inference?: number;
  /**
   * `status` only.
   */
  claims_open?: number;
  /**
   * `status` only. The same three over the node's **edges** — its links' `claim_tag` (#857). BESIDE the node counts and never folded into them: a node's claims are measured over its text, an edge is in no node's text and belongs to two nodes at once, so one merged figure would answer about a denominator nobody chose. A consumer wanting the corpus-wide total adds the two. NOT required: a CLI released before this field emits a measurement without it, and its output must still validate.
   */
  edge_claims_verified?: number;
  /**
   * `status` only. The same three over the node's **edges** — its links' `claim_tag` (#857). BESIDE the node counts and never folded into them: a node's claims are measured over its text, an edge is in no node's text and belongs to two nodes at once, so one merged figure would answer about a denominator nobody chose. A consumer wanting the corpus-wide total adds the two. NOT required: a CLI released before this field emits a measurement without it, and its output must still validate.
   */
  edge_claims_inference?: number;
  /**
   * `status` only. The same three over the node's **edges** — its links' `claim_tag` (#857). BESIDE the node counts and never folded into them: a node's claims are measured over its text, an edge is in no node's text and belongs to two nodes at once, so one merged figure would answer about a denominator nobody chose. A consumer wanting the corpus-wide total adds the two. NOT required: a CLI released before this field emits a measurement without it, and its output must still validate.
   */
  edge_claims_open?: number;
  /**
   * `index-status` only. Which index answered: `local`, this checkout's `.yidam/index/`, or `shared`, the main worktree's, read by a linked worktree with none of its own when it was built at HEAD or an ancestor (RFC-0069). Null when no index was found. The values are the MCP contract's `index_source`, less `remote`, which only a server reports.
   */
  index_source?: "local" | "shared" | null;
  /**
   * `index-status` only. The directory read, absolute. Null exactly when `index_source` is. A fact about this machine, so no golden holds one non-null.
   */
  index_path?: string | null;
  /**
   * `index-status` only. Whether the index carries a readable `meta.json`. An index present but undescribable is a different state from an absent one, and the two used to render as the same sentence.
   */
  meta_present?: boolean;
  /**
   * `index-status` only. Unix timestamp the index was built at, or null when there is no metadata. The raw stamp and not an age: an age is a function of when you ask, so humanizing it is the consumer's affordance.
   */
  built_at?: number | null;
  /**
   * `index-status` only. Civil date of `built_at`, so a reader need not convert one.
   */
  built?: string | null;
  /**
   * `index-status` only. Embedding model the index was built with.
   */
  model?: string | null;
  /**
   * `index-status` only. Vector width.
   */
  embedding_dim?: number | null;
  /**
   * `index-status` only. Distinct nodes in the index at build time. Compare against `status.nodes` to see what the index does not carry.
   */
  node_count?: number | null;
  /**
   * `index-status` only. Rows in the index at build time, one per passage (RFC-0060): above `node_count` by the passages long nodes were split into. An index built before passages reports `node_count` here. NOT required: a CLI released before this field emits a report without it, and its output must still validate.
   */
  row_count?: number | null;
  /**
   * `index-status` only. Corpus files modified since the build. This is the freshness verdict — a consumer MUST NOT recompute it by comparing mtimes itself.
   */
  stale_nodes?: number;
  /**
   * `sangha` only. Whether this repository runs collective governance, keyed on registered electors rather than on the directory: the template ships `sangha/` with a placeholder table, so a directory test would report every derived repository as collective from its genesis commit.
   */
  collective?: boolean;
  /**
   * `sangha` only. Rows of `electors.md` whose branch cell names a `ma/*` ref — which is what PROTOCOL.md defines an elector as.
   */
  electors?: {
    name: string;
    /**
     * `ma/<name>`.
     */
    branch: string;
    /**
     * The Role cell verbatim, markdown and all — it is prose written for a reader.
     */
    role: string;
    /**
     * `agent`, `human`, or empty. RFC-0012 attestation. NOT REQUIRED, and neither is any other attestation field: a released CLI predates the columns and a registry written before them fills none of them in, so empty means the registry does not say.
     */
    kind?: string;
    /**
     * What produced this seat's positions, for an agent elector. Recorded, never privileged — Article II governs weight and Article III governs record.
     */
    model?: string;
    /**
     * The model's version, where the model name does not carry it.
     */
    version?: string;
    /**
     * A hash of the agent's operative configuration, never the configuration itself.
     */
    config?: string;
    /**
     * The seat's SSH public key in `authorized_keys` form — the key, not a fingerprint. `yidam lint` generates the `gpg.ssh.allowedSignersFile` from this column at verification time, so the registry is the trust root and a key it does not carry verifies nothing. Empty means the seat declares its commits unverifiable.
     */
    key?: string;
    /**
     * Whether the branch exists, local or remote-tracking. A registered elector whose branch is gone is a state to report, not an error.
     */
    branch_present: boolean;
  }[];
  /**
   * An array in `sangha` (files in `positions/`, attributed by the `<elector>-<question>` naming convention); an integer in `status` (standing `ma/*` elector positions, which are neither active work nor drift — divergence from the baseline is what a position is for). This key was declared TWICE in this object until #650, once per shape. Every JSON parser keeps the last, so the `status` declaration and its description were dead text that read as coverage. `items` below therefore applies to the `sangha` shape only.
   */
  positions?:
    | {
        /**
         * Repository-relative path.
         */
        file: string;
        /**
         * The registered elector whose name prefixes the stem, or EMPTY when none does. Empty is the honest answer for a position filed under a name the table does not carry; a consumer must not guess a nearest match.
         */
        elector: string;
        /**
         * The rest of the stem — the `rigpa/<evolution>` a resolution would carry.
         */
        question: string;
      }[]
    | number;
  /**
   * `sangha` only. Files in `resolutions/`, with their frontmatter.
   */
  resolutions?: {
    file: string;
    /**
     * The `evolution:` field, falling back to the filename stem.
     */
    evolution: string;
    date: string;
    /**
     * The `ma/*` tips the record says it read, verbatim — `ma/<elector>@<short-hash>`.
     */
    tips: string[];
    /**
     * Whether `rigpa/<evolution>` still exists. False is routine: the branch is often deleted once the resolution lands on the baseline.
     */
    branch_present: boolean;
    /**
     * The `synthesized-by:` seats — the elector branch that executed the synthesis, and a list where it was joint. NOT REQUIRED, deliberately: a released CLI predates the field and emits records without it, and every record written before the field existed carries none, so a consumer that assumes it is present is wrong in two directions at once. Empty means the record does not say who executed it, which is a finding (`resolution-executor-unrecorded`) rather than a gap in this report.
     */
    synthesized_by?: string[];
    /**
     * The `independence:` field, verbatim — what the record says `electors.md` distinguishes among the seats it read, drawn from a closed vocabulary of `distinct-seats` / `shared-configuration` / `unrecorded`. NOT REQUIRED, for the reason `synthesized_by` records: a released CLI predates the field and every record written before it existed carries none. Verbatim and not normalized — a value outside the vocabulary is reported (`resolution-independence-mismatch`) rather than rounded to the nearest member, because `unrecorded` is not a weaker `distinct-seats`: a consumer MUST NOT read it as either other value. An empty string means the record does not say, which is the state of every record written before the field existed and is itself distinct from a stated `unrecorded`.
     */
    independence?: string;
    /**
     * The `rounds:` field, verbatim — how many times the resolution loop ran. A STRING, not an integer, and not normalized: `rounds: two` or `rounds: 0` is a finding (`resolution-deliberation-unrecorded`) rather than a value to coerce, and parsing it would turn a malformed count into an absent one. NOT REQUIRED, for the reason `synthesized_by` records. An empty string means the record does not say.
     */
    rounds?: string;
    /**
     * The `positions:` the record says the loop read, verbatim — `positions/<elector>-<question>.md`, relative to the sangha directory. With `rounds`, what makes Article III checkable rather than asserted: `tips` names the commits read, and this names the claims contested and by whom. NOT REQUIRED, for the reason `synthesized_by` records. Empty means the record does not say, which is a finding (`resolution-deliberation-unrecorded`).
     */
    positions?: string[];
  }[];
  /**
   * `vocabulary` only. Repository-relative path of the GRAPH.md the **When** prose came from — the vendored copy if present, otherwise `yidam/prelude/`. Empty when neither exists, which costs the prose and nothing else.
   */
  source?: string;
  /**
   * `vocabulary` only. The closed commit vocabulary. MEMBERSHIP AND KIND COME FROM THE PARITY-CERTIFIED LISTS, not from GRAPH.md — a consumer that decided either from the document would be a second implementation of the predicate `lint --commits` gates on, and could green-light a verb CI then reports. GRAPH.md supplies `when` and nothing else.
   */
  verbs?: {
    verb: string;
    kind: CommitKind;
    /**
     * The **When** cell, or empty when no GRAPH.md was found.
     */
    when: string;
  }[];
  /**
   * `vocabulary` only. Disagreements between GRAPH.md's tables and the certified lists. Empty is the expected state; non-empty means the document and the predicate that gates on it have come apart, and the document is the half that is wrong. Three SDKs carry a comment saying the constants are 'kept in sync with prelude/GRAPH.md' and nothing verified it until this field existed.
   */
  drift?: string[];
  /**
   * `vocabulary --check` only. One subject line checked the way `lint --commits` checks a committed one — but before the commit exists. Absent when `--check` was not given.
   */
  subject?: {
    /**
     * The first line, trimmed. Only the subject is checked.
     */
    text: string;
    /**
     * Everything before the first `: `, exactly as `classify_commit` reads it. Empty when there is no such separator.
     */
    verb: string;
    /**
     * How `classify_commit` files a commit, declared once. Total by construction: operational verbs are the explicitly marked case and every other verb is epistemic, which the Dafny spec proves — so this enum is closed and a third member would be a change to that proof, not to a report.
     */
    kind: "epistemic" | "operational";
    /**
     * Whether `verb` is in the closed vocabulary. Not the verdict: `violations` is. A subject git wraps — `Revert "…"`, `fixup! …` — has no `verb` of its own and is judged by the subject inside the wrapper, and a generated merge subject or a commit staging only `[object]` paths is not judged at all, so either can read `false` here with no violations.
     */
    recognized: boolean;
    violations: {
      /**
       * `scope-suffix` is separate from `unrecognized-verb` deliberately: it has a known cause and a known double cost, and saying so is the difference between a squiggle that teaches and one that scolds.
       */
      rule: "scope-suffix" | "unrecognized-verb" | "no-verb";
      /**
       * Read from the `unrecognized-verb` lint check rather than restated — all three rules are sub-cases of it. A consumer MUST render at this severity and not escalate: a squiggle stricter than the gate is an editor asserting a verdict nobody agreed to.
       */
      severity: "error" | "warn" | "info";
      message: string;
    }[];
  };
  /**
   * `corpus-index`, `graph`, `neighbors` and `rename`. Repository-relative corpus root; every `node` path in the same report is relative to it. Emitted so a consumer never hardcodes `.yidam/corpus` — the one path in this contract a repository could plausibly move. On a `rename` of a catalog entry it is `.yidam`, and `from` and `to` are `catalog/<name>.md`: the moved file is always `corpus_dir/to`.
   */
  corpus_dir?: string;
  /**
   * `graph` only. The `.ont.yml` class definitions: what each kind of node is, the properties it may carry, and the relationships it may enter into. Exposed so a client offering completion reads the ontology from the CLI rather than becoming a second reader of a shape the prelude defines.
   */
  classes?: {
    /**
     * The `class:` field, falling back to the filename — `<class>.ont.yml` is what `graph-check` matches an instance's directory against, so the filename is the name that actually governs.
     */
    class: string;
    label: string;
    description: string;
    properties: {
      name: string;
      /**
       * `string`, `date`, `ref`, or `text` — see `yidam schema`.
       */
      type: string;
      description: string;
      /**
       * Whether every instance of the class must carry this property. `missing-property` GATES on a property declared true, so this is the difference between a finding that fails CI and one that does not — but `false` folds two answers the check treats differently, a property declared `required: false` and one that says nothing, and `omission` keeps them apart (#1155). Until #606 this report did not carry it, which made `yidam schema`'s claim that one declaration decides both, 'so the editor and the gate cannot disagree', false along this route. Always emitted, including when false: a client is versioned independently of the binary it talks to, so an absent field would be ambiguous between 'this property is optional' and 'this binary predates the field', and it was the second reading that produced a page claiming every property in a corpus was optional. Absent therefore means the binary is older than the field, never that the property is optional.
       */
      required?: boolean;
      /**
       * What `missing-property` does when an instance omits this property: `gates` for a property declared `required: true` (an error that fails CI), `reported` for one whose class says nothing about `required` (a warning), and `licensed` for one declared `required: false` (not reported at all). It exists because `required` folds the last two into one `false`, and a client labelling every `false` property as reported was wrong about the licensed ones (#1155). It names the verdict rather than the declaration, so a client never re-derives the check's rule. Always emitted, for `required`'s reason: absent means the binary predates the field, and a client falls back to `required`.
       */
      omission?: "gates" | "reported" | "licensed";
    }[];
    edges: {
      relationship: string;
      /**
       * A CLASS NAME, not a path. `holds` on `person` targets `tenure`, and the instances it may point at are the ones in `tenure/`.
       */
      target: string;
      /**
       * `out` when instances of this class author the link, `in` when the other side does. A class declares both so its neighbourhood is readable from one file.
       */
      direction: string;
      description: string;
    }[];
  }[];
  /**
   * `neighbors` only. The class of the node asked about.
   */
  class?: string;
  /**
   * `neighbors` only. The label of the node asked about.
   */
  label?: string;
  /**
   * `neighbors` only. The description of the node asked about, so a client rendering a neighbourhood need not fetch the centre separately.
   */
  description?: string;
  /**
   * `neighbors` only. The node asked about, corpus-relative, or EMPTY when the id matched nothing. An unknown node is an empty report rather than an error: a client asking about the file you happen to have open must not have to know in advance whether it is a corpus node.
   */
  node?: string;
  /**
   * `neighbors` only. The maximum hop count the traversal was allowed — not a per-neighbour distance; see `neighbors[].hops`.
   */
  depth?: number;
  /**
   * `neighbors` only. Breadth-first over the UNDIRECTED view of the graph: each reached node once, at its shortest hop distance, carrying the direction of the edge it was reached by. This is the same traversal `serve --mcp`'s `neighbors` tool performs, from the same function — half the interesting connections are inbound, which is why `orphan-in` exists at all.
   */
  neighbors?: {
    /**
     * Corpus-relative.
     */
    node: string;
    class: string;
    label: string;
    description: string;
    relationship: string;
    /**
     * Relative to the node it was reached FROM, which at hops > 1 is not the node asked about.
     */
    direction: "out" | "in";
    /**
     * Distance from the start. Named `hops` and not `depth` because `depth` on this report is the maximum that was asked for, and a field meaning two things in one document is how a consumer reads one as the other. (`serve --mcp` calls it `depth` in its own output; that contract is unchanged.)
     */
    hops: number;
    /**
     * False for an `.ont.yml` target or a broken edge — reached, and not a corpus node. Reported rather than dropped: a neighbourhood that hid them would disagree with the graph it came from.
     */
    is_node: boolean;
  }[];
  /**
   * `rename` only. Source, relative to `corpus_dir`: `<class>/<name>.yml` for a node, `catalog/<name>.md` for a catalog entry.
   */
  from?: string;
  /**
   * `rename` only. Destination, relative to `corpus_dir`; may name a different class. A catalog entry stays in the catalog.
   */
  to?: string;
  /**
   * `rename` only. Whether anything was written. False for `--dry-run`, and false whenever `blocked` is non-empty. A client rendering a preview MUST check this before claiming the rename happened.
   */
  applied?: boolean;
  /**
   * `rename` only. The file move — one entry, or none when blocked.
   */
  moves?: {
    /**
     * Repository-relative.
     */
    file: string;
    /**
     * 1-based. Zero on a `moves` entry, which is about a file rather than a line in one.
     */
    line: number;
    from: string;
    to: string;
  }[];
  /**
   * `rename` only. Every `target:` rewrite, located — or, for a catalog entry, every edge `source:` rewrite in the spelling its author used (stem or path) and every markdown link under `.yidam/` that resolved to the entry. Covers inbound edges AND the moved node's own outgoing links, which need re-relativizing when it changes class — a same-directory target survives a rename inside one class and breaks the moment the node leaves it.
   */
  edits?: {
    /**
     * Repository-relative.
     */
    file: string;
    /**
     * 1-based. Zero on a `moves` entry, which is about a file rather than a line in one.
     */
    line: number;
    from: string;
    to: string;
  }[];
  /**
   * `rename` only, and only for a corpus node. The `moved-from:` line written into the moved node, naming the path it left the way a `target:` would, so that `orphan-in`'s uncited age and `due`'s open-question age carry across the move instead of restarting. `file` is the node's new path. `line` is where the line lands: past the end when the node declared none, or the line it replaces, whose value `from` then carries (empty otherwise). Absent for a catalog entry and when blocked.
   */
  moved_from?: {
    /**
     * Repository-relative.
     */
    file: string;
    line: number;
    from: string;
    to: string;
  };
  /**
   * `rename` only. Markdown references to the old path. Reported, NEVER rewritten: RFC-0014 scopes the rewrite to the corpus walk, and the difference between 'renamed' and 'renamed and quietly broke the README' is whether anybody was told.
   */
  unhandled?: {
    file: string;
    line: number;
    /**
     * The line, trimmed.
     */
    text: string;
  }[];
  /**
   * `rename` only. Why this cannot proceed. Non-empty means NOTHING was touched and the process exits nonzero.
   */
  blocked?: string[];
  /**
   * `rename` only. A suggested subject in the closed commit vocabulary. `migrate:` and not `rename:` — RFC-0014's own example uses a verb that is in no list, which `lint --commits` reports and which makes `classify_commit` file an operational commit as Epistemic.
   */
  commit_subject?: string;
  /**
   * `regen --check` only. REGEN blocks whose committed content is not what their generator produces. Empty when `passed`. Computed by running the same generator list `yidam regen` runs — a check walking its own list would be the third list that command exists to have prevented.
   */
  stale?: {
    /**
     * Repository-relative path of the file holding the block.
     */
    file: string;
    /**
     * The `<command>` in `<!-- REGEN: yidam <command> -->`.
     */
    generator: string;
  }[];
  /**
   * `regen --check` only. REGEN blocks whose command names no generator this binary has, so nothing writes them and `yidam regen` cannot make them current. Empty when `passed`. Read off the tracked markdown rather than off the generator list — a block nothing is pushed into is a block nothing records, which is how a misspelled marker name stayed green (#1062). Only `yidam `-prefixed commands are judged: the marker namespace is open and a derived repository's own binary writes its own blocks.
   */
  unclaimed?: {
    /**
     * Repository-relative path of the file holding the block.
     */
    file: string;
    /**
     * The `<name>` in `<!-- REGEN: yidam <name> -->`, with the `yidam ` stripped.
     */
    generator: string;
  }[];
  /**
   * `index-verify` only. `match` — same vector space. `known-delta` — a DIFFERENT space, within a bound that runtime declared in advance; passes, and is meant to be surfaced rather than swallowed. `mismatch` — beyond every bound, or the wrong shape; degrade to keyword search. `unverifiable` — the index carries no witness, which is every index built before the field existed and is not the consumer's fault.
   */
  verdict?: "match" | "known-delta" | "mismatch" | "unverifiable";
  /**
   * `index-verify` only. The index directory examined.
   */
  index?: string;
  /**
   * `index-verify` only. From the index's `embed.config.json`.
   */
  model_id?: string;
  /**
   * `index-verify` only. The sentence the witness was taken on. Absent when the index carries no witness.
   */
  probe?: string;
  /**
   * `index-verify` only. Largest per-element difference between the provider's embedding and the witness. Absent when nothing was compared — a wrong dimensionality is not drift.
   */
  max_drift?: number;
  /**
   * `index-verify` only. The bound applied, which is the runtime's `known_delta` when one was granted.
   */
  tolerance?: number;
  /**
   * `index-verify` only. The runtime a `known-delta` verdict was granted to. A consumer that does not name itself is held to the strict tolerance — inferring a bound would make the declaration a discount.
   */
  runtime?: string;
  /**
   * `index-verify` only. What is wrong, and what to do about it.
   */
  problems?: string[];
  /**
   * `log` only. Which filter produced this view.
   */
  filter?: "all" | "epistemic" | "operational";
  /**
   * `log` only. Commits in the range BEFORE filtering — a filtered view still reports what it hid.
   */
  total?: number;
  /**
   * `log` only. Epistemic commits in the range, before filtering.
   */
  epistemic?: number;
  /**
   * `log` only. Operational commits in the range, before filtering.
   */
  operational?: number;
  /**
   * `log` only. The commits the filter admitted.
   */
  entries?: {
    hash: string;
    short: string;
    /**
     * How `classify_commit` files a commit, declared once. Total by construction: operational verbs are the explicitly marked case and every other verb is epistemic, which the Dafny spec proves — so this enum is closed and a third member would be a change to that proof, not to a report.
     */
    kind: "epistemic" | "operational";
    /**
     * Leading verb, or empty when the subject carries none.
     */
    verb: string;
    subject: string;
    author: string;
    /**
     * Author date, ISO-8601.
     */
    date: string;
  }[];
  /**
   * `kuten check` only. What this corpus declared its practice is aimed at (RFC-0028), and where its own history disagrees. Advisory by construction: the command exits zero however far a corpus has drifted, so a consumer MUST NOT read a divergence as a gate failure.
   */
  kuten?: {
    /**
     * Whether this repository declares a kuten at all. `false` is a supported state and not a finding — it was all eighteen corpora the layer was measured over.
     */
    held: boolean;
    /**
     * The kuten named by `.yidam/decisions/kuten.yml`.
     */
    name?: string | null;
    /**
     * The revision the decision record names.
     */
    declared_revision?: number | null;
    /**
     * The revision of the profile actually vendored under `.yidam/.vendor/prelude/kuten/`.
     */
    vendored_revision?: number | null;
    /**
     * Whether those two disagree. A comparison across revisions is annotated rather than silently made, on the same rule that refuses a cross-PROTOCOL_VERSION diff.
     */
    revision_skew: boolean;
    /**
     * Why there is nothing to compare, when a kuten is declared and cannot be read.
     */
    unresolved?: string | null;
    /**
     * What the repository's VENDORED prelude could have produced. A metric that prelude could not have produced is reported as `vintage` and never as divergence — a repository whose GRAPH.md has no `phase` verb has not stopped running phases.
     */
    vintage: {
      has_phase_verb: boolean;
      vocabulary_is_closed: boolean;
      /**
       * Whether a vendored GRAPH.md was found at all. False is absence, not age.
       */
      graph_present: boolean;
    };
    /**
     * What this repository's own history and working tree show.
     */
    measurement: {
      /**
       * Commits with an authored subject in the corpus register; git-generated merge subjects are excluded, because nobody chose their verb, and so is every commit whose paths all lie in `[object] paths` (RFC-0062). `commits`, `phase_commits`, `off_vocabulary_commits` and `suffixed_commits` all count this scoped population, which is the one the bands judge. Each unscoped figure is the field plus its `object_only_` twin; with no object declared every twin is 0.
       */
      commits: number;
      phase_commits: number;
      off_vocabulary_commits: number;
      /**
       * How many of `off_vocabulary_commits` are a recognised verb wearing a conventional-commits scope — `phase(a-flat-grant):` rather than a coinage. A STRICT SUBSET, never a second population: every commit counted here is counted there too, and a consumer that subtracts one from the other is performing the suffix-strip GRAPH.md forbids. It exists so a divergence can say which of the two mistakes it found (#673) — one derived repository's 49% was 80 suffixed verbs and 130 genuine coinages, and a reader shown only the share concludes the vocabulary is being ignored rather than misspelled. NOT required, for the reason `open_questions` gives: a CLI released before this field emits a measurement without it, and its output must still validate.
       */
      suffixed_commits?: number;
      /**
       * Authored commits left out of every scoped count because each of their paths lies in `[object] paths`. 0 where no object is declared. Counted apart from the scoped fields so the unscoped reading is always recoverable (RFC-0062 §2.4). NOT required, for the reason `open_questions` gives: a CLI released before this field emits a measurement without it, and its output must still validate.
       */
      object_only_commits?: number;
      /**
       * The phase commits among `object_only_commits`. Counted apart from the scoped fields so the unscoped reading is always recoverable (RFC-0062 §2.4). NOT required, for the reason `open_questions` gives: a CLI released before this field emits a measurement without it, and its output must still validate.
       */
      object_only_phase_commits?: number;
      /**
       * The off-vocabulary commits among `object_only_commits`. Counted apart from the scoped fields so the unscoped reading is always recoverable (RFC-0062 §2.4). NOT required, for the reason `open_questions` gives: a CLI released before this field emits a measurement without it, and its output must still validate.
       */
      object_only_off_vocabulary_commits?: number;
      /**
       * The suffixed commits among `object_only_off_vocabulary_commits`, so `suffixed_commits` stays a subset of `off_vocabulary_commits` on both readings. Counted apart from the scoped fields so the unscoped reading is always recoverable (RFC-0062 §2.4). NOT required, for the reason `open_questions` gives: a CLI released before this field emits a measurement without it, and its output must still validate.
       */
      object_only_suffixed_commits?: number;
      nodes: number;
      /**
       * Null where the corpus has no instance nodes.
       */
      median_node_lines?: number | null;
      /**
       * Instance nodes the corpus currently holds open as questions, by the same predicate `open-questions` reports. NOT required: a CLI released before the question-pressure slot emits a measurement without it, and its output must still validate. Corpus state and deliberately not `open:` commits — two of the six repositories the layer was extracted from have written none while holding open questions in the corpus.
       */
      open_questions?: number;
    };
    /**
     * One entry per claim the kuten makes that this repository can be read against, including the ones that conform: a metric that vanishes when it agrees cannot be told from one that was never read. A slot the profile leaves unpopulated produces no entry. Most entries read a measured value against a declared band, and `declared` is then that band; the `question-pressure` entry reads a declared *kind* instead, so `declared` is the kind's name and `measured` is a count or a reason. Both shapes carry the same `verdict` set, and a consumer keys on `metric` rather than on which shape it is.
     */
    findings: {
      /**
       * The kuten slot the band belongs to.
       */
      slot: string;
      /**
       * Stable identifier; the prose is free to change.
       */
      metric: string;
      /**
       * `divergent` is a question for a person, never a defect. `vintage` means the vendored prelude could not have produced the metric. `unmeasurable` means there was nothing to measure.
       */
      verdict: "conforming" | "divergent" | "vintage" | "unmeasurable";
      /**
       * The band this metric declares, rendered in the SAME unit as `measured` — `12%–27%`, not `0.12–0.27`. The two halves of one comparison came from different renderers until #696, so a row read `declared 0.12–0.27  measured 12%` and the reader had to convert one of them to make the comparison the row is.
       */
      declared: string;
      /**
       * What this repository reads, and on divergence WHICH SIDE it fell on — `11.9% (below 12%)`. The verdict compares an `f64` and this is rounded, so without the relation a divergent row could render a value that reads as inside the band it is being reported outside of — and a consumer parsing this back and re-testing it against `declared` would reach the opposite verdict (#696). Higher precision shrinks that window and cannot close it: any finite precision has a value that rounds onto an endpoint from outside.
       */
      measured: string;
      /**
       * The question a person is asked, where there is one. Absent on a conforming metric.
       */
      question?: string | null;
    }[];
    /**
     * Whether every measured metric is inside its band. NOT an exit code: this command exits zero either way.
     */
    conforming: boolean;
    /**
     * The links between the corpus and the artifact `[object] paths` declares, in both directions (RFC-0028 §6, #577). Relative markdown links only, read over the tracked `.md`, `.rs` and `.ts` files the object globs claim and outside every authorship region — not `cites:`, which no derived corpus writes (Erratum 6). Null where no object is declared, and where the held kuten declares the corpus `projected`, because the projection is then the coupling. Read whether or not a kuten is held. NOT required, for the reason `suffixed_commits` gives: a CLI released before this field emits a report without it, and its output must still validate.
     */
    coupling?: {
      /**
       * The globs `[object] paths` declares, as read.
       */
      object: string[];
      /**
       * The artifact files read: tracked, claimed by a glob, one of `.md`/`.rs`/`.ts`, and outside every authorship region.
       */
      files: number;
      inbound: CouplingLinks;
      outbound: CouplingLinks1;
      nodes: CouplingReach;
      decisions: CouplingReach1;
      catalog: CouplingReach2;
    } | null;
  };
  /**
   * `score` only. One range of commits read against the criteria this corpus's kuten declares, or against the template's own where it holds none (RFC-0028, #286). Advisory by construction: there is no overall number, no verdict and no band, and the command exits zero however it reads — a consumer MUST NOT derive a pass or a failure from any row. A range spanning a kuten revision is refused rather than answered.
   */
  score?: {
    /**
     * The range as the caller wrote it. Always explicit: there is no default, because a default range produces a number about a scope nobody chose.
     */
    range: string;
    /**
     * Full sha of the commit the range starts after.
     */
    base: string;
    /**
     * Full sha of the commit it ends at. The corpus is reconstructed at both, from git objects, never from the working tree.
     */
    tip: string;
    /**
     * Whether the criteria are this corpus's own. `false` means they are the template's, which is every repository today — the same criteria, and a different answer to whose selection they are. True exactly when `source` is `kuten`; a consumer that needs to know WHY the criteria are the template's MUST read `source`, because `false` covers three different situations (#695).
     */
    held: boolean;
    /**
     * Why the criteria are what they are. A closed set, so a consumer branches without matching prose. `kuten`: the range's tip declares one and its profile is vendored. `no-kuten`: neither the tip nor the working tree declares one. `predates-adoption`: the tip declares none and the working tree does — the criteria are correctly the template's because the range is older than the decision, and the repository does hold a kuten. `unreadable-profile`: the tip declares one and no profile is vendored for it, which `kuten check` and `doctor` also report. Only `no-kuten` was ever reported honestly by `held` alone.
     */
    source: "kuten" | "no-kuten" | "predates-adoption" | "unreadable-profile";
    /**
     * The kuten the criteria came from. Non-null exactly when `held` is true; what the range's tip DECLARED is `declared`, which does not have to agree.
     */
    kuten?: string | null;
    /**
     * The revision of the profile the criteria came from, as the record at the TIP of the range names it — read from that commit's tree rather than from the working copy. Non-null exactly when `held` is true: the record used to carry `held: false` beside a revision, which says two contradictory things about one range (#695).
     */
    revision?: number | null;
    /**
     * What the decision record at the range's TIP names, whatever the criteria became. Present alongside the criteria under `source: kuten`, and alone under `source: unreadable-profile` — which is the disagreement stated rather than resolved.
     */
    declared?: {
      name: string;
      revision: number;
    } | null;
    /**
     * The kuten the WORKING TREE declares, where that is not what the range was scored against. The one fact in this report about now rather than about the range, and it is here for the one distinction the range cannot make on its own: a tip that predates an adoption and a corpus that has adopted nothing are the same commit-shaped absence.
     */
    holds_now?: string | null;
    /**
     * Whether the vendored profile the criteria were read from is at another revision than that record names. Annotated, never silently resolved.
     */
    revision_skew: boolean;
    /**
     * Authored, non-merge commits in the range — the same reader and the same merge predicate `lint --commits` uses.
     */
    commits: number;
    /**
     * Corpus instances present at the tip and absent at the base.
     */
    added_nodes: number;
    /**
     * Exactly one per declared criterion, in declaration order — including the ones that could not be read. A criterion that vanished when it had no denominator could not be told from one that was never declared.
     */
    rows: {
      /**
       * The criterion id, as the profile's `rubric.criteria` declares it. A consumer keys on this; the prose may change.
       */
      criterion: string;
      /**
       * What the criterion read. `unmeasurable` is NOT zero: it means there was no denominator, and a consumer that reads it as 0.00 is asserting something the report declined to.
       */
      reading: {
        kind: "measured" | "unmeasurable";
        /**
         * The share, 0.0–1.0. Present only when `kind` is `measured`.
         */
        value?: number;
        /**
         * The reading as a reader is shown it, e.g. `2 of 3 (67%)`. Present only when `kind` is `measured`.
         */
        shown?: string;
        /**
         * Why there was nothing to read. Present only when `kind` is `unmeasurable`.
         */
        why?: string;
      };
      /**
       * The items the reading was drawn from, marked — capped, with a final line saying how many were left out. Non-empty whenever `kind` is `measured`: a number with nothing behind it is a claim nobody can check.
       */
      evidence: string[];
      /**
       * What a reader has to know to read the number correctly — the legibility precondition beside `register`, or the reason `questions` is never a threshold.
       */
      note?: string | null;
    }[];
    /**
     * The judged questions, when `--brief` asked for them. Scored by nothing, and deliberately: each turns on whether an assertion is warranted, and no corpus fact decides it. Empty otherwise.
     */
    questions: string[];
  };
  /**
   * `catalog-fetch` only. One row per catalog entry the run had something to do about — an entry declaring no `location:`, or only locations nothing can follow, is absent rather than present-and-empty. NOT `entries`, which this contract already gives to `log`'s commits. Two lists per row and they answer different questions: `obtained` is what was followed and cached, `skipped` is a declared address that was not, with the reason. A `kind: address` sitting beside a followable location is in neither — a records office is a real way to hold a source, not a failure, and reporting it as one on every run would train a reader to ignore the list.
   */
  fetched?: {
    /**
     * The catalog entry's file stem.
     */
    entry: string;
    /**
     * One row per location followed. Empty under `--dry-run` is not the empty case — the rows are present with `sha256` blank, because a dry run resolves every address and fetches none.
     */
    obtained: {
      /**
       * Index into the entry's own `location:` list — the same index an artifact record's `from:` carries.
       */
      location: number;
      /**
       * The address as the entry writes it, template slots unfilled.
       */
      declared: string;
      /**
       * What was actually followed: the bound URL, or the resolved path. Equal to `declared` for a plain `url`.
       */
      followed: string;
      /**
       * Content address of the bytes, 64 lowercase hex. Empty under `--dry-run`.
       */
      sha256: string;
      /**
       * Size as fetched. 0 under `--dry-run`.
       */
      bytes: number;
      /**
       * The server's `Content-Type`, parameters stripped. Null where the server said nothing and for every local file — guessing from an extension would put a claim in a committed record that no server made.
       */
      media_type: string | null;
      /**
       * Whether the cache already held these bytes. True is a re-fetch that found no change, and is why the run may write no commit.
       */
      cached: boolean;
      /**
       * Where `vault push` would send it, as prose, from the config's `holds`. NEVER a credential, and never a claim that anything was sent — a fetch ends at the local cache.
       */
      route: string;
      /**
       * `<pack>@<version>` of the source pack that resolved an identifier location (RFC-0048 §5). Absent for every other kind.
       */
      pack?: string;
      /**
       * Requests a scheme's `paginate` made to read the whole answer (#1341). `sha256` is the digest of the pages joined into one document. Absent for every location whose scheme does not page.
       */
      pages?: number;
    }[];
    /**
     * Declared addresses this run did not follow, each with the reason. The commonest is a `url_template` whose slots nothing bound, and the reason names them so a reader can supply `--bind`.
     */
    skipped: {
      location: number;
      declared: string;
      /**
       * Written for someone reading a terminal, and repairs itself where it can.
       */
      why: string;
    }[];
    /**
     * The commit this run authored, or null when nothing changed. A re-run that finds the same bytes, or a `used-by` already in agreement, writes no commit at all — an empty commit asserting that a fetch happened would be provenance invented rather than recorded. Always an OPERATIONAL verb: `catalog-fetch` writes `refresh:` and `catalog-reconcile` writes `reconcile:`, and the writer checks its own subject against `classify_commit` before it runs, because RFC-0026 confines a run to operational commits and sends every epistemic one to a proposal branch.
     */
    commit: {
      /**
       * Short sha of the commit, as this repository's object count makes it.
       */
      sha: string;
      /**
       * The subject line, leading verb included.
       */
      subject: string;
    } | null;
    /**
     * `catalog-fetch` only, and absent from older producers. Locations whose publisher answered with something other than success (RFC-0048 §5). Recorded as a finding and not retried: a 403 is the publisher's answer, and nothing steps around it. Any row here makes the run exit nonzero; the entry's other locations are still fetched and recorded.
     */
    refused?: {
      location: number;
      declared: string;
      /**
       * The URL asked. NEVER carries a credential, even one the pack sends in the query string.
       */
      followed: string;
      /**
       * The HTTP status.
       */
      status: number;
      why: string;
    }[];
    /**
     * `catalog-fetch --archive` only; empty without it. One row per identifier location whose pack declares `archive = "wayback"`, and with `--archive=url` per `kind: url` location, looked up through the Wayback availability API, which is a read: the run never asks the archive to capture anything.
     */
    archived?: {
      location: number;
      /**
       * The address looked up.
       */
      followed: string;
      /**
       * The date the lookup asked for the capture nearest, `YYYY-MM-DD`: the `retrieved:` of the record holding the bytes the run obtained, or of the location's latest record where the publisher declined (#1366). Null when the run obtained bytes the entry did not hold, or the location has no record, and the archive answered with the capture nearest today.
       */
      asked: string | null;
      /**
       * The nearest successful capture's date, `YYYY-MM-DD`. Null when the archive holds none.
       */
      captured: string | null;
      /**
       * The `wayback:<timestamp>/<url>` location appended to the entry, or null when there was no capture.
       */
      added: string | null;
      /**
       * The digest of the capture's `id_` bytes, read until the added location holds a record of them, and recorded as its artifact in the same commit (#1589) unless the entry already holds the same bytes with no `from:`. Null when there was no capture, or it could not be read.
       */
      sha256: string | null;
      /**
       * Whether the capture's bytes equal the source's: the bytes this run obtained, or the location's latest record where the publisher declined. Null when either digest is unknown, or the source is paginated and its capture is one page. False never fails the run: a PDF source captured as an HTML page differs by design.
       */
      matches: boolean | null;
    }[];
    /**
     * Repo-relative paths of every corpus node citing this entry, when this run recorded a NEW VERSION of it — a digest from a location the entry already held a digest from (#1200). Empty on a first fetch, on a digest from a location nothing held yet, and on a run that recorded nothing. Every citing node rather than a subset, since the version did not exist until this run and nothing can yet have been read against it. `due`'s `superseded` clock carries the list after this report is gone.
     */
    superseded?: string[];
  }[];
  /**
   * `catalog-extract` only. One row per catalog entry recording an artifact with a reading still to take — a PDF with no text reading, an artifact whose source pack declares an `extract` transform it has not been read by (#1318), or a zip with a member its location names under `members:` and no reading of it (#1351); an entry with none is absent. NOT `entries`, which this contract already gives to `log`'s commits. `read` is what was read and recorded, `skipped` is an artifact that was not, with the reason.
   */
  extracted?: {
    /**
     * The catalog entry's file stem.
     */
    entry: string;
    /**
     * One row per artifact or member read. Under `--dry-run` the rows are present with `sha256` and `bytes` null, because a dry run reads nothing.
     */
    read: {
      /**
       * Content address of the PDF read.
       */
      artifact: string;
      /**
       * Content address of the reading, as recorded under the PDF's `text:`. Null under `--dry-run`.
       */
      sha256: string | null;
      /**
       * What took the reading, as its record's `by` says it: the extractor and version, a source pack's transform as `<pack>@<version>/<transform>@sha256:<hash>`, or `unzip` for a member read out of a zip.
       */
      extractor: string;
      /**
       * The file inside the zip this row reads, by its path there (#1351). Absent for a row that reads the whole artifact.
       */
      member?: string;
      /**
       * Size of the reading. Null under `--dry-run`.
       */
      bytes: number | null;
    }[];
    /**
     * Artifacts this run did not read: not in this machine's cache, corrupt there, unreadable by the extractor or transform, read as no text at all (which is what a scanned page gives), or asking for a transform this build does not run.
     */
    skipped: {
      artifact: string;
      /**
       * Written for someone reading a terminal.
       */
      why: string;
    }[];
    /**
     * The `extract:` commit this run authored, or null when nothing was recorded.
     */
    commit: {
      sha: string;
      subject: string;
    } | null;
  }[];
  /**
   * `catalog-reconcile` only. Retained for compatibility (RFC-0053 §2.4): always empty. The command is retired and writes nothing; it stays one release so a guard reading this field keeps passing, and the release after removes it. The row shape below is what it reported when it rewrote lists. NOT `entries`, which this contract already gives to `log`'s commits.
   */
  reconciled?: {
    /**
     * The catalog entry's file stem.
     */
    entry: string;
    /**
     * The disagreement BEFORE the repair, in the same shape `catalog-audit` reports it and from the same `used_by_drift` the gate calls. Two consumers of one function, so a repair cannot come to disagree with the gate about what drifted.
     */
    drift: {
      /**
       * Names the list claimed that no node citing this entry carries. Removed by the repair — keeping them would be keeping the drift.
       */
      claimed_not_citing: string[];
      /**
       * Nodes that cite this entry and the list omitted. Added.
       */
      citing_not_claimed: string[];
    };
    /**
     * The list as it now stands, written entry-relative (`../corpus/…`) because that is the form every hand-written list uses. Under `--dry-run` this is what would be written.
     */
    used_by: string[];
    /**
     * The commit this run authored, or null when nothing changed. A re-run that finds the same bytes, or a `used-by` already in agreement, writes no commit at all — an empty commit asserting that a fetch happened would be provenance invented rather than recorded. Always an OPERATIONAL verb: `catalog-fetch` writes `refresh:` and `catalog-reconcile` writes `reconcile:`, and the writer checks its own subject against `classify_commit` before it runs, because RFC-0026 confines a run to operational commits and sends every epistemic one to a proposal branch.
     */
    commit: {
      /**
       * Short sha of the commit, as this repository's object count makes it.
       */
      sha: string;
      /**
       * The subject line, leading verb included.
       */
      subject: string;
    } | null;
  }[];
  /**
   * `cycle` only. What this repository is owed — the same eight clocks `due` reports, read through `due`'s own reader and carrying the same item shape. A consumer MUST NOT render a `due` clock as a fault: a corpus with three expired sources is owed, not unhealthy, and `doctor` is the report that answers whether anything is wrong.
   */
  owed?: {
    /**
     * Stable identifier: `index`, `catalog`, `questions`, `phases`, `superseded`, `asks`, `coverage` or `template`.
     */
    id: string;
    /**
     * What the clock measures, in the form a person would ask it.
     */
    question: string;
    /**
     * `due` is not a defect. A corpus with three expired sources is owed, not unhealthy — `doctor` is the report that answers whether anything is wrong. `undeclared` means this corpus set no interval, so the clock can never come due; `declined` means it set none deliberately, having decided against the thing the clock measures, and `record` names where it argued that; `unmeasurable` means it set one and there is nothing to measure against, which is a gap in bookkeeping rather than a stale subject; on `template` alone it may instead mean the origin could not be read, which is a fact about the network the run had. A consumer MUST NOT render `declined` as `undeclared`: the first is a decision and the second is its absence. `unbuildable` is the one state that is a fact about the BINARY rather than about the corpus: the act that would discharge the clock is compiled out of this build, so the clock has no subject here whatever the corpus declared. Only the `index` clock can reach it — `index-build` is behind a feature the released binary does not carry, and every other clock measures something a `reports` build can both read and discharge. It is NOT `due`, and the difference is whether the row can ever change: a clock nothing available to the reader can discharge reads red on every run forever, which is the state two derived repositories met before this one existed (#1061) — one declined the clock to silence it and one wrote a comment into its config explaining the row. Its `remedy` is an install rather than a corpus act, and a consumer MUST NOT count it as owed. A light build holding an index pulled from elsewhere is NOT in this state: reading a `meta.json` needs no feature, so the staleness verdict there is the real one.
     */
    state: "due" | "ok" | "undeclared" | "declined" | "unmeasurable" | "unbuildable";
    /**
     * The declaration that sets this clock's interval, as a reader would write it. Null when this corpus has not set one. The catalog clock reads `[catalog] ttl_days` and each entry's own `ttl_days:`; the other four read `[due]` keys.
     */
    interval?: string | null;
    /**
     * Subjects past the interval. Zero on every state but `due`.
     */
    overdue: number;
    /**
     * What was measured, whether or not an interval applies.
     */
    detail: string;
    /**
     * The command or act that discharges it — or, on an `undeclared` clock, the key that would set it. Null when there is nothing to do, which includes every `declined` clock: nothing discharges a decision not to want the thing.
     */
    remedy?: string | null;
    kind?: ActKind;
    /**
     * The command `remedy` names, as words to execute with the envelope's `root` as the working directory. Present exactly when `kind` is `tool`; a `person` remedy never carries one.
     */
    argv?: string[];
    /**
     * The decision record behind a `declined` clock, repo-relative under `.yidam/decisions/`. Null on every other state. The record is required for a decline to be honoured at all — a `[due.declined]` entry naming a record the repository does not hold leaves the clock in the state it was really in, so a decline cannot rot into a silenced row.
     */
    record?: string | null;
    /**
     * The owed subjects by name, one line each, on a clock whose subjects a person must visit, and only when `due`. On `superseded` each line names a node, the entry it cites and how long ago that entry gained its latest version; on `asks`, an ask and the re-pin it stands against; on `coverage`, a member of a declared universe that no instance carries, and the entry that listed it. Every subject is listed, however many; a text rendering may show fewer. Absent otherwise, which a consumer MUST read as an empty list.
     */
    subjects?: string[];
  }[];
  /**
   * `cycle` only. The manifest's steps whose committed receipt does not stand at the tip, read by `yidam run --dry-run`'s planner against the local checkout — owed as surely as a due clock, and the half `due` has no clock for. Empty when the corpus declares no manifest, and empty when the planner refused (see `stale_steps_unread`). NOT SPELLED `stale`, which this schema already gives to `regen --check` as an array of REGEN blocks.
   */
  stale_steps?: {
    /**
     * The capability's name in `.yidam/capabilities.toml`.
     */
    step: string;
    /**
     * `unknown` is a step a stale step earlier in the plan would write the inputs of. Both are owed: `yidam run` invokes the first and then decides the second. `fresh` never appears — a fresh step is not owed.
     */
    freshness: "stale" | "unknown";
    /**
     * The planner's reason, verbatim.
     */
    because: string;
  }[];
  /**
   * `cycle` only. Why the manifest's steps could not be planned — a detached HEAD, or a step this binary cannot invoke — as the planner's own refusal. Null when they were planned AND when there is no manifest: a corpus declaring no capabilities has asked for nothing. A consumer MUST NOT read an empty `stale_steps` beside a non-null value as nothing owed.
   */
  stale_steps_unread?: string | null;
  /**
   * `cycle --since-last` only, and absent unless it was asked: what moved since the last round `cycle --record` kept for this branch. Each half is two lists, what arrived and what left, with no total across them. An empty delta is nothing having moved; a branch with no record has no `since_last` at all, and says so in `since_last_unread`.
   */
  since_last?: {
    /**
     * The full commit the recorded round stood on.
     */
    head: string;
    /**
     * Gating findings by `(gate, check, node)`, accepted or not, as a multiset: a second occurrence of one finding is an arrival.
     */
    findings: {
      arrived: {
        /**
         * `lint` or `graph-check`.
         */
        gate: string;
        check: string;
        node: string;
      }[];
      left: {
        /**
         * `lint` or `graph-check`.
         */
        gate: string;
        check: string;
        node: string;
      }[];
    };
    /**
     * Clock ids newly due, and clock ids discharged.
     */
    due: {
      arrived: string[];
      left: string[];
    };
    /**
     * Step names newly stale, and step names brought current.
     */
    stale_steps: {
      arrived: string[];
      left: string[];
    };
    /**
     * Inquiry refs started, and refs no longer in flight.
     */
    in_flight: {
      arrived: string[];
      left: string[];
    };
    /**
     * Local `propose/*` heads opened, and heads gone.
     */
    proposals: {
      arrived: string[];
      left: string[];
    };
  };
  /**
   * `cycle --since-last` only, and absent unless it was asked: why there is no delta — no round recorded for this branch, or a record this binary cannot read. A consumer MUST NOT read its presence as nothing having moved.
   */
  since_last_unread?: string;
  /**
   * `cycle` only. The UNSETTLED inquiry refs — `phases`' own rows, filtered to the state it publishes. A settled phase and a standing position are both absent, and that filter is the point: a list headed "in flight" holding refs that have landed is the conflation `phases` grew a `state` column to undo.
   */
  in_flight?: {
    /**
     * The phase's name, humanized from its ref.
     */
    name: string;
    /**
     * The git ref, local or remote-tracking.
     */
    ref_name: string;
    /**
     * Author of the ref's tip commit, or `unknown`.
     */
    owner: string;
    /**
     * Date of the first commit unique to this ref, or `—` where git could give none. A row with no readable start date is unmeasured, not fresh.
     */
    started: string;
    /**
     * Commits unique to this ref, against the baseline.
     */
    commits: number;
  }[];
  /**
   * `cycle` only. What a gate would fail on TODAY — not everything the corpus was ever forgiven. `lint`'s findings are read through the baseline ratchet, so a corpus carrying accepted findings is not listed as blocked by them: the gate is green over it, and listing them would tell a reader that a repository doing exactly what it agreed to has things to fix. NOT SPELLED `blocked`, which this schema already gives to `rename` as an array of strings with a different meaning — one property name cannot carry two types, which is the defect `rejected.code` was repaired for. The half is still called *blocked* in the text report; this is what the list holds.
   */
  blocking?: {
    /**
     * Which gate says so. A consumer branches on this to know which command repairs it.
     */
    gate: "lint" | "graph-check";
    /**
     * The check or condition, as that gate names it. `lint`'s check id, or `graph` for a graph-check finding.
     */
    check: string;
    /**
     * The node or file, repo-relative, where there is one.
     */
    node?: string | null;
    /**
     * Why it blocks. For `lint` this distinguishes a finding introduced against the baseline, an entry past its expiry, and a stale entry the run no longer reproduces — three different events with three different repairs.
     */
    detail: string;
    /**
     * What discharges it.
     */
    remedy: string;
  }[];
  /**
   * `cycle` only. What to do next, one act per half, ranked BY THE SERVER. No kuten profile declares a phase order — what one declares is a SET of phase types — so a consumer MUST NOT read this as an order the corpus specified. It is an argument the report makes and states, and it is a list to disagree with.
   */
  next?: {
    /**
     * Which half produced the act — what a reader needs in order to argue with the ranking. Deliberately not a severity or a priority number: the ordering is an argument rather than a measurement, and a number would present it as the second thing.
     */
    source: "blocked" | "owed" | "in_flight" | "practice";
    /**
     * The command or act, as a reader would run or perform it. For an owed clock this is `due`'s own remedy verbatim — seven of the eight clocks are discharged by a person, not by a command this binary runs.
     */
    act: string;
    /**
     * The finding, clock or declaration behind it.
     */
    why: string;
    kind?: ActKind;
    /**
     * The act as words to execute, with the envelope's `root` as the working directory — the command does not repeat it. Present exactly when `kind` is `tool`. Every value this binary emits starts `yidam`.
     */
    argv?: string[];
    /**
     * What the act is about: the nodes a gate's findings name, the ref in flight, the steps a run would invoke, the nodes a clock lists. Empty when the act is about the corpus as a whole.
     */
    targets?: string[];
  }[];
  /**
   * `cycle` only. What this corpus declared its work is aimed at, from its kuten. A corpus holding none is the SUPPORTED state and every repository measured so far is in it; `undeclared` is non-null there and says which of three ways this half is short. A consumer MUST NOT render a null `kuten` as a defect.
   */
  practice?: {
    /**
     * The profile this corpus adopted. Null where it holds none.
     */
    kuten?: string | null;
    /**
     * The revision its decision record names. Null where it holds no record.
     */
    revision?: number | null;
    /**
     * The phase types the profile declares — A SET, NOT A SEQUENCE. Nothing declares an order between them, and a consumer that renders this as steps has invented one.
     */
    phase_types: string[];
    /**
     * The kind of question this practice presses toward. It creates pressure and authors nothing.
     */
    question_pressure?: string | null;
    /**
     * Why this half is short, in words. Non-null in three different cases — no decision record, a record naming a profile this repository does not hold, and a record or profile that did not parse — and they are different facts rather than one absence.
     */
    undeclared?: string | null;
  };
  /**
   * `policy check` only. Every decision the compiled rule set answers, and where its rule came from. The set is what `policy eval --decision` and `policy gate` accept, so a caller that named one wrong can correct without a second call — the reason `check_subject` carries the closed vocabulary in the MCP contract.
   */
  decisions?: {
    /**
     * `family/name`, as `--decision` spells it.
     */
    decision: string;
    /**
     * `local` where `.yidam/policy/` overrides the rule, `inherited` where the compiled-in default answers. RFC-0024's premise is that a corpus may override a built-in guard by package name, so which of the two answered is the first thing a reader needs.
     */
    origin: "local" | "inherited";
    /**
     * The overriding file's path, relative to the corpus root. **Null iff `origin` is `inherited`** — a rule nobody overrode has no file to name, and reporting the binary's own path would be a location nobody can open.
     */
    source: string | null;
  }[];
  /**
   * `policy check` only. Rego builtins a committed policy calls that this build does not carry. Empty is the healthy case and is emitted rather than omitted, because absent and empty are the two answers a consumer must not have to tell apart. A non-empty list makes `ok` false.
   */
  disallowed_builtins?: {
    /**
     * The policy package naming the builtin.
     */
    policy: string;
    /**
     * The builtin as called, e.g. `http.send`.
     */
    call: string;
  }[];
  /**
   * `policy check` only. Whether every rule compiled and named only builtins this build carries. **Not a verdict about the repository** — that is `policy gate`'s `allow`, and `check` exits zero whatever it finds. NOT `passed`, which this contract gives one meaning across `graph-check`, `regen --check`, `index-verify` and `doctor`.
   */
  ok?: boolean;
  /**
   * `policy eval` and `policy gate`. The decision that was asked, echoed as `family/name`, so a verdict read out of a log says which question produced it.
   */
  decision?: string;
  /**
   * `policy eval` and `policy gate`. The verdict. `gate` additionally exits nonzero when this is false; `eval` never does — it answers a question rather than enforcing one, which is RFC-0024's division and the reason somebody can ask why a push was refused without reproducing the refusal.
   */
  allow?: boolean;
  /**
   * `policy eval` and `policy gate`. Every deny rule that fired, with its message. Empty when `allow` is true. Always the explained form in JSON, whatever `--explain` says: that flag shapes the text rendering, and a machine reader that had to ask twice for the rule names would be the surface RFC-0024 wrote `eval` to remove.
   */
  deny?: {
    /**
     * The Rego rule that fired, fully qualified.
     */
    rule: string;
    /**
     * What it says, as the rule wrote it. Carried verbatim — never reworded here.
     */
    msg: string;
  }[];
  /**
   * `policy test` only. Every `test_*` rule in every `*_test.rego`, in the order they ran. A rule evaluating to `Undefined` is reported as a failure and never as a skip: a body that did not hold asserted nothing, and counting that as a pass is how a suite comes to cover less than it claims.
   */
  tests?: {
    /**
     * The test rule, fully qualified.
     */
    rule: string;
    /**
     * Whether it held. Nested, and unrelated to the envelope-level `passed` this contract reserves for a gate verdict.
     */
    passed: boolean;
    /**
     * Why it did not hold. Null on a pass.
     */
    detail: string | null;
    /**
     * The `family/name` decision this test exercises, or null where the rule names none. A decision no test covers is a rule nobody has checked.
     */
    covers: string | null;
    /**
     * Whether `.yidam/policy/` replaced the rule under test. A failure with this true is counted in `changed_by_override` rather than in `failed`: the corpus changed the rule on purpose, and reporting that as a broken test would make every intentional override look like a defect.
     */
    overridden: boolean;
  }[];
  /**
   * `policy test` only. Tests that held. **NOT `passed`**, and the name is the whole of the reason: this contract gives `passed` one meaning — the gate verdict, a boolean — so a count under that name would be one key carrying two types for two commands. It emitted `passed` until #893, which nothing caught because the command emitted no envelope and no golden read it.
   */
  passing?: number;
  /**
   * `policy test` only. Tests that failed against a rule `.yidam/policy/` replaced. A strict subset of the failures and **not** part of `failed`, so the two never double-count: `failed` is what is broken, this is what the corpus changed on purpose.
   */
  changed_by_override?: number;
  /**
   * `count` only. The query as it was given, echoed. A number is only readable against the question it answers, and the consumer holding it — a REGEN block, a report on disk — is rarely the caller that chose the argument.
   */
  query?: string;
  /**
   * `count` only. How many nodes satisfied the query's final step — the whole traversal, not a projected page: `--limit` bounds what is shown and never what is counted. **Null, never 0, when `rejected` is present.** A query that could not run has not counted zero of anything, and a consumer that cannot tell a diagnosis from a measurement would publish the diagnosis as a figure.
   */
  matched?: number | null;
  /**
   * `count` only. Why the query could not typecheck against this corpus, or null when it ran. Present-and-null rather than absent, so a consumer testing for the key cannot read `accepted` off a binary too old to say.
   */
  rejected?: {
    /**
     * Which step of the path the rejection is about, zero-based; null for a rejection about the query as a whole rather than one of its steps.
     */
    step?: number | null;
    /**
     * The rejection's stable identifier — `unknown-class`, `parse`, `unlicensed-hop` and the rest. A plain string rather than an enum on purpose: that vocabulary is frozen in `mcp/tools.json`, and a copy here would be a fourth place to forget.
     */
    code: string;
    /**
     * The diagnosis in prose, naming what was wrong and, where the corpus affords one, the nearest name it does declare.
     */
    message: string;
  } | null;
  /**
   * `derive check` only. The `[derive] paths` globs this repository declares, as written. Empty means none are declared: the report is then empty and the command passes, which is every repository before RFC-0045.
   */
  derive_paths?: string[];
  /**
   * `derive check` only. How many span records the `[[derive.records]]` mappings matched (RFC-0063 §3.5). A consumer holds it above zero, which also fails a CLI too old to emit it.
   */
  records?: number;
  /**
   * `derive check` only. One entry per artifact under `[derive] paths`, sorted by path, then one per span record the `[[derive.records]]` mappings matched, and one per pointer that matched nothing in a file. The command fails when any entry carries an `error` finding; an `info` finding never fails it.
   */
  derivations?: {
    /**
     * The artifact, repository-relative. For a span record, the JSON file holding it.
     */
    path: string;
    /**
     * A span record's pointer with its wildcards filled, such as `/models/3/formulation`, or the pointer as written where it matched nothing. Absent for an artifact.
     */
    record?: string;
    /**
     * The artifact's `claim:`, whitespace collapsed, or null where it states none.
     */
    claim: string | null;
    /**
     * The artifact's `reach:` as written — `public`, `attributed` or `internal` when it is one — or null where it declares none.
     */
    reach: string | null;
    /**
     * The weakest standing over every located span. `open` for an artifact citing nothing, because nothing beneath it licenses more. Null when no cited span could be located, so no standing was read.
     */
    tier: "open" | "inference" | "verified" | null;
    /**
     * Each `cites:` entry, in the artifact's order.
     */
    spans: {
      /**
       * The cited node's corpus key, `class/name`.
       */
      node: string | null;
      /**
       * The span as the artifact quotes it.
       */
      span: string | null;
      /**
       * The weakest claim overlapping the span, or `open` where none does. Null when the span could not be located.
       */
      standing: "open" | "inference" | "verified" | null;
    }[];
    findings: {
      /**
       * Stable id, one of the nine `derive-*` findings RFC-0045 §3.5 names.
       */
      check: string;
      /**
       * `error` for every finding but `derive-refusal-candidate`, which is `info` and proposes a `refuses:` declaration a person decides on.
       */
      severity: "error" | "warn" | "info";
      /**
       * The cited node the finding is about, where it is about one.
       */
      node?: string;
      /**
       * What was found and the repair. Prose, free to change.
       */
      message: string;
    }[];
  }[];
  /**
   * `record --fold` only (#1018). What one fold counted from `.yidam/record/calls.jsonl` into the tracked `.yidam/consumption.json`, and the `refresh:` commit that holds it. The file is never truncated: `from` and `to` are the bytes this fold counted, and a line past `to` is the next fold's.
   */
  fold?: {
    /**
     * The file folded from, always `.yidam/record/calls.jsonl`.
     */
    record: string;
    /**
     * The tracked file folded into, always `.yidam/consumption.json`.
     */
    path: string;
    /**
     * Calls this fold counted. Zero when there was nothing new.
     */
    calls: number;
    /**
     * Lines this fold counted as unreadable.
     */
    unreadable: number;
    /**
     * The first byte of the file this fold counted.
     */
    from: number;
    /**
     * One past the last byte this fold counted: the end of the last complete line it read. Equal to `from` when nothing was counted.
     */
    to: number;
    /**
     * The `refresh:` commit this fold authored, or null when there was nothing new to fold.
     */
    commit: {
      sha: string;
      subject: string;
    } | null;
    /**
     * Calls counted in the history after this fold, every fold together.
     */
    total: number;
  };
  /**
   * `record` only (#1019). What `serve --mcp` was asked, read from `.yidam/record/calls.jsonl` — the file `[serve] record` makes a server append one line per `tools/call` to. `declared` and `present` are two fields and not one because an absent file and an empty one are different findings: the first says nothing was recorded, the second that a server kept the record and was asked nothing. A consumer MUST NOT read `calls: 0` with `present: false` as a corpus nobody read.
   */
  consumption?: {
    /**
     * Repo-relative path of the record, always `.yidam/record/calls.jsonl`.
     */
    path: string;
    /**
     * Whether `.yidam/config.toml` declares `[serve] record = true` now. A record can be present and undeclared: it is what was kept while the key was set.
     */
    declared: boolean;
    /**
     * Whether the file exists. False means nothing was recorded, which is not a claim that nothing was asked.
     */
    present: boolean;
    /**
     * Lines read as calls. Excludes `unreadable`. Counts the committed fold and the file past it together (#1018).
     */
    calls: number;
    /**
     * How many of `calls` were read from `.yidam/consumption.json`, the tally `record --fold` commits (#1018), rather than from the file. Zero in a corpus that has never folded. With `present: false` and a nonzero value, the file is gone and the history still says what it held.
     */
    folded: number;
    /**
     * Lines that were not a JSON object naming a `tool` — a torn last line from a killed server is the likeliest. Counted rather than dropped, so a record of N calls is never reported as N−1.
     */
    unreadable: number;
    /**
     * Distinct corpus commits the calls answered from.
     */
    commits: number;
    /**
     * Earliest `at` in the record, seconds since the epoch. Null with no timestamped line.
     */
    first_at: number | null;
    /**
     * Latest `at` in the record, seconds since the epoch. Null with no timestamped line.
     */
    last_at: number | null;
    /**
     * Calls that answered with zero rows and were not rejected — a rejection is a caller's error, not a hole in the corpus. A line with no `rejected` key, from an older writer, counts as not rejected.
     */
    empty: number;
    /**
     * Distinct `args_digest`s among `empty`: how many questions came back empty, where `empty` counts how many times.
     */
    empty_questions: number;
    /**
     * Calls whose outcome was `error`.
     */
    errors: number;
    /**
     * One row per tool the record names, busiest first, ties by name.
     */
    tools: {
      tool: string;
      /**
       * The MCP contract's tier for the tool. Null for a name the contract does not carry, which is what a mistyped tool name records as.
       */
      tier: string | null;
      calls: number;
      errors: number;
      empty: number;
      /**
       * Calls served degraded. NULL, not zero, for a tool no line reported the key for: a tool with no degraded path and a tool that never degraded are different facts.
       */
      degraded: number | null;
      /**
       * Calls that reported `degraded` at all. `degraded == reported_degraded` with both non-zero is every call served degraded — the corpus has never answered this tool from semantic search.
       */
      reported_degraded: number;
      /**
       * Median latency in milliseconds, the lower middle on an even count. Null with no line carrying `ms`.
       */
      p50_ms: number | null;
    }[];
    /**
     * Tools the MCP contract carries that no line names, in the contract's order. Every tier is listed, including ones this corpus does not back.
     */
    never_called: string[];
    /**
     * Which of this corpus's nodes `retrieve` has returned, from the `node_ids` record lines carry (#1020) — the consumption side of `lint`'s uncited check. NULL when no answered `retrieve` line carries `node_ids`: an older writer recorded a row count, and the complement of a count is not computable. A consumer MUST NOT read null as every node unreached.
     */
    reach: {
      /**
       * This corpus's own nodes at the working tree. A dependency's are not counted.
       */
      nodes: number;
      /**
       * How many of `nodes` some recorded `retrieve` returned.
       */
      returned: number;
      /**
       * `nodes` no recorded `retrieve` returned, sorted by id.
       */
      unreached: string[];
      /**
       * Answered `retrieve` calls whose line names the nodes it returned.
       */
      calls: number;
      /**
       * Answered `retrieve` calls whose line does not, from an older writer. They are in no count here, so a nonzero value means `unreached` may list a node one of them returned.
       */
      unrecorded: number;
    } | null;
  };
  /**
   * `cluster status` only. The corpus slug whose runs are reported: the `yidam.dev/corpus` label, and the `CronWorkflow` named `yidam-<corpus>`.
   */
  corpus?: string;
  /**
   * `cluster status` only. The corpus's recent runs, newest first, read from the records Argo keeps on each `Workflow` — never from a log. Operational status: nothing here is committed (#1236).
   */
  runs?: {
    /**
     * The `Workflow`'s name. Null for a run Argo has deleted, seen only through its `CronWorkflow`'s last scheduled time.
     */
    workflow: string | null;
    /**
     * Argo's phase for the run, or `unknown`.
     */
    phase: string;
    /**
     * The cron tick the run was created for. Absent for a run created any other way.
     */
    scheduled?: string;
    /**
     * When Argo started the run.
     */
    started?: string;
    /**
     * When Argo finished the run.
     */
    finished?: string;
    /**
     * Why nothing below the run is known: its `Workflow` is gone, or its node status was offloaded.
     */
    unknown?: string;
    admission: {
      /**
       * `not-asked` is a one-shot `Workflow`, which has no `admit` task.
       */
      state: "admitted" | "not-admitted" | "not-asked" | "running" | "unknown";
      /**
       * The admission record's own reason, or why there is none.
       */
      because: string;
      /**
       * The `due` clocks that were owed, by id.
       */
      owed: string[];
      /**
       * The steps whose receipt did not match the tip.
       */
      stale: string[];
      /**
       * The `propose/*` branches open when admission ran.
       */
      open_proposals?: number;
      /**
       * `[cluster] max_open_proposals`, when declared.
       */
      max_open_proposals?: number;
    };
    /**
     * In the order the workflow runs them.
     */
    steps: {
      /**
       * The capability.
       */
      step: string;
      /**
       * What became of it. `landed` is an operational commit on the branch and `proposed` an epistemic one on `propose/*`; `fresh` and `unchanged` landed nothing because there was nothing to land; `refused` is the lander declining, with its reason; `failed` is a pod that failed without a record; `unknown` is a node whose record is missing, never a guess.
       */
      outcome:
        | "landed"
        | "proposed"
        | "fresh"
        | "unchanged"
        | "refused"
        | "failed"
        | "running"
        | "not-reached"
        | "unknown";
      /**
       * The ref the commit landed on, `main` or `propose/<pin>`.
       */
      target?: string;
      /**
       * The sha that landed. Absent when the ref already held the result.
       */
      commit?: string;
      /**
       * For a proposal, whether its branch is still on the remote. Absent when the refs were not read.
       */
      still_open?: boolean;
      /**
       * The lander's refusal, or what is missing for an outcome that is `failed` or `unknown`.
       */
      reason?: string;
    }[];
    gathers: {
      /**
       * The gather, by name.
       */
      gather: string;
      /**
       * The gather's landing, as its step record names it: `gather/<name>`.
       */
      step?: string;
      /**
       * What became of it. `landed` is an operational commit on the branch and `proposed` an epistemic one on `propose/*`; `fresh` and `unchanged` landed nothing because there was nothing to land; `refused` is the lander declining, with its reason; `failed` is a pod that failed without a record; `unknown` is a node whose record is missing, never a guess.
       */
      outcome:
        | "landed"
        | "proposed"
        | "fresh"
        | "unchanged"
        | "refused"
        | "failed"
        | "running"
        | "not-reached"
        | "unknown";
      /**
       * The ref the commit landed on, `main` or `propose/<pin>`.
       */
      target?: string;
      /**
       * The sha that landed. Absent when the ref already held the result.
       */
      commit?: string;
      /**
       * For a proposal, whether its branch is still on the remote. Absent when the refs were not read.
       */
      still_open?: boolean;
      /**
       * The lander's refusal, or what is missing for an outcome that is `failed` or `unknown`.
       */
      reason?: string;
      /**
       * Every peer the survey decided on or an asker answered for, by name.
       */
      peers: {
        /**
         * The peer package.
         */
        peer: string;
        /**
         * `gather`'s own outcome word, or `unknown` for an asker that left no record.
         */
        outcome: string;
        /**
         * Why, when the survey or the missing record gives one.
         */
        reason?: string;
      }[];
    }[];
  }[];
  /**
   * `cluster status` only. The branch and the proposal branches on the remote, by `ls-remote`. Null when no `--remote` was given and `[cluster] remote` is unset.
   */
  refs?: {
    /**
     * The remote read.
     */
    remote: string;
    /**
     * The branch runs land on.
     */
    branch: string;
    /**
     * The branch's tip. Null when it is not on the remote or the remote could not be read.
     */
    tip: string | null;
    /**
     * Every `propose/*` branch, without `refs/heads/`.
     */
    proposals: string[];
    /**
     * Why the remote could not be read. The runs are reported regardless.
     */
    unread?: string;
  } | null;
  /**
   * `source check` only. Every source pack the repository holds, authored packs first, each sorted by name (RFC-0048). A vendored pack an authored one of the same name replaces is listed, with `shadowed`, and is not enabled.
   */
  packs?: {
    /**
     * The pack's directory name.
     */
    name: string;
    /**
     * `[pack].version` as written. Null when the manifest did not parse.
     */
    version: string | null;
    /**
     * `authored` is under `.yidam/sources/`; `vendored` is under `.yidam/.vendor/sources/`, copied by `yidam-vendor-update` from a `prelude_sources` pin.
     */
    origin: "authored" | "vendored";
    /**
     * The pack directory, relative to the repository root.
     */
    path: string;
    /**
     * The schemes the manifest declares, sorted. Empty when it did not parse.
     */
    schemes: string[];
    /**
     * Entries in the manifest's `[fixtures]` table.
     */
    fixtures: number;
    /**
     * A vendored pack an authored pack of the same name replaces.
     */
    shadowed: boolean;
  }[];
  /**
   * `source check` only. What is wrong with a pack or a `prelude_sources` pin. Not `findings`, which is `check-diff`'s shape. Any `error` fails the check; `passed` is the verdict, and a consumer MUST NOT recompute it from this list.
   */
  pack_findings?: {
    /**
     * `error` fails the check. `info` does not, and is said for a vendored pack an authored one shadows. Never `warn`.
     */
    severity: "error" | "warn" | "info";
    /**
     * The pack, or the pin, the finding is about.
     */
    pack: string;
    /**
     * The file to open, relative to the repository root.
     */
    path: string;
    message: string;
  }[];
  /**
   * `source list` only. Whether this build runs a pack's `describe` and `extract`, which needs the `source-transforms` feature.
   */
  transforms_run?: boolean;
  /**
   * `source list` only. Every enabled source pack, authored packs first, each sorted by name (RFC-0048). A shadowed vendored pack, or one whose manifest does not parse, is not enabled and not here. Not `packs`, which is `source check`'s list of every pack held.
   */
  source_packs?: {
    /**
     * The pack's directory name.
     */
    name: string;
    /**
     * `[pack].version`.
     */
    version: string;
    /**
     * As in `packs[].origin`.
     */
    origin: "authored" | "vendored";
    /**
     * The pack directory, relative to the repository root.
     */
    path: string;
    /**
     * `[pack].description`.
     */
    description: string | null;
    /**
     * Each scheme the pack declares, sorted.
     */
    schemes: {
      scheme: string;
      /**
       * The catalog `type:` an entry of this scheme is written with.
       */
      type: string;
      /**
       * Whether the scheme names a `describe` transform.
       */
      describe: boolean;
      /**
       * Whether the scheme names an `extract` transform.
       */
      extract: boolean;
      /**
       * The schemes its `then` leads to.
       */
      then: string[];
    }[];
    /**
     * The scheme `[search]` answers in, or null when the pack declares no search.
     */
    search: string | null;
    /**
     * What the pack's `[transport]` asks of the environment, and whether the environment has it. Never a value.
     */
    needs: {
      /**
       * `[transport] contact`. A pack that requires one is not asked without `YIDAM_CONTACT`.
       */
      contact: "required" | "optional";
      /**
       * Whether `YIDAM_CONTACT` is set and not blank.
       */
      contact_set: boolean;
      /**
       * Each `[transport] auth` variable.
       */
      auth: {
        /**
         * The variable's name.
         */
        var: string;
        /**
         * Whether it is set and not blank. Its value is never reported.
         */
        set: boolean;
      }[];
      /**
       * `[transport] blocked`: why the pack says its publisher cannot be asked at all.
       */
      blocked: string | null;
    };
    /**
     * Whether every need is met, so a request to the publisher would be made here.
     */
    ready: boolean;
  }[];
  /**
   * `source list` only. Pack directories whose manifest does not parse, and so are not enabled. `source check` says why.
   */
  unreadable_packs?: string[];
  /**
   * `source search` only. What a pack's `[search]` endpoint answered. Nothing is written.
   */
  search?: {
    pack: string;
    /**
     * The query as given.
     */
    query: string;
    /**
     * The scheme every candidate's identifier is in.
     */
    scheme: string;
    /**
     * The template with `{query}` percent-encoded and `{limit}` bound. Asked only when `answered` is `network`.
     */
    url: string;
    /**
     * `fixture` under `--offline`: the pack's recorded `[search] fixtures` answered, and nothing was asked.
     */
    answered: "network" | "fixture";
    /**
     * In the order the endpoint answered, at most `--limit`. A result with no value at the `id` path is passed over.
     */
    candidates: {
      /**
       * `scheme:local-id`, the value `source add` takes.
       */
      identifier: string;
      /**
       * The result's title as the publisher wrote it.
       */
      title: string | null;
    }[];
  };
  /**
   * `source add` only. One row per identifier, each a draft catalog entry written `obtained: false`. Every identifier resolves or none is written, so a refusal is an error and not a row.
   */
  drafts?: {
    /**
     * The entry's file stem, and its `name:`.
     */
    entry: string;
    /**
     * `.yidam/catalog/<entry>.md`.
     */
    path: string;
    /**
     * False under `--dry-run`.
     */
    written: boolean;
    /**
     * The `catalog:` commit that wrote the entry. Null under `--dry-run`. Operational: it adds a source anchor and says nothing about what the source holds.
     */
    commit: {
      /**
       * Short sha of the commit.
       */
      sha: string;
      /**
       * `catalog: <entry> from <identifier>`.
       */
      subject: string;
    } | null;
    /**
     * The identifier asked for, trimmed.
     */
    identifier: string;
    /**
     * The pack whose scheme resolved it.
     */
    pack: string;
    /**
     * The identifier first, then each one its `then` chain reached. Each is a `kind: identifier` location of the entry.
     */
    locations: {
      identifier: string;
      /**
       * The address the scheme's template binds it to.
       */
      url: string;
      /**
       * Null for the identifier asked for; otherwise the identifier whose describe named it, and the key.
       */
      via: string | null;
    }[];
    /**
     * What the identifier's `describe` made of its answer, or the pack's templates alone and why.
     */
    draft: {
      /**
       * The source's title. The entry's slug and heading come from it.
       */
      name: string | null;
      /**
       * The catalog `type:`. The scheme's own when the transform names none.
       */
      kind: string | null;
      date: string | null;
      description: string | null;
      /**
       * Further identifiers the answer names, as `[key, value]` pairs a `then.from` of `describe.<key>` reads.
       */
      identifiers: string[][];
      /**
       * The draft fields left empty.
       */
      unfilled: string[];
      /**
       * Why no transform filled them, when none ran.
       */
      why: string | null;
      /**
       * The transform that filled the draft.
       */
      by: string | null;
    };
    /**
     * `[defaults] ttl_days`, which the entry is written with.
     */
    ttl_days: number | null;
    /**
     * Where the describe's answer came from. Null when no describe was asked.
     */
    answered: "network" | "fixture" | null;
    /**
     * Why a `then` was not followed, one line each.
     */
    unfollowed: string[];
  }[];
}
export interface ALintCheck {
  id: CheckId;
  /**
   * `lint` only.
   */
  title: string;
  /**
   * `lint` only. The severity the check is DECLARED at — not necessarily the severity of its findings, which is `violations[].severity` below. A consumer filtering gate-relevant findings on this field drops every escalated one: `missing-property` is declared `warn` and gates the properties a class marked `required: true`, and any corpus-state check can escalate on age. This said 'Only `error` gates, and only `error` is ever baselined' until #650, which contradicted the field 30 lines down and is how the consumers named there came to read the wrong one.
   */
  severity: "error" | "warn" | "info";
  /**
   * `lint` only. Why the check exists.
   */
  rationale: string;
  /**
   * `lint` only. Whether `[lint] escalate_after` can reach this check's findings AT ALL. Escalation needs two things at once — a threshold the corpus declared, and a finding carrying `age` — so a check that dates none of its findings is unreachable by every value of the threshold, and most checks here are. That went unstated until #774, when a derived corpus declined to arm `escalate_after` because the bulk of its findings were a shape no value of it could ever have escalated. Read it here rather than inferring it from `violations[].age`: a check that found nothing emits no violations and is still the kind of check that ages. True does NOT mean anything escalated — that is `violations[].severity` — only that this check is in the population the threshold acts on.
   */
  escalation_eligible: boolean;
  /**
   * `lint` only. Every finding the check raised, including none — a check that vanishes when it passes cannot be told from one that never ran.
   */
  violations: {
    /**
     * The violation's identity, exactly as the baseline compares it. Unaffected by `span`.
     */
    node: string;
    detail: string;
    /**
     * Whether the committed baseline already records this violation. ONLY MEANINGFUL WHEN severity IS error — the baseline records error-severity violations and nothing else, so a warn or info violation is always false here. Read this together with `severity`, never alone.
     */
    in_baseline: boolean;
    /**
     * This violation's effective severity, and the one that decides whether it GATES. Normally the check's, and not always: `Check::severity_of` returns the finding's own override or an age escalation before falling back to the check's level, so `missing-property` is declared `warn` and raises the one property a class marked `required: true` to `error`. Read it here and never from the check — that is also what `in_baseline` above means by 'together with `severity`'. A consumer that reads the check's level renders the one finding whose whole purpose is to gate as advisory: that is what the VS Code extension, the web editor and the LSP each did until #655, undetected because no fixture held a violation whose severity was not its check's. `reports/basic` now does. This field went undeclared until #645 because the contract gate compared only the envelope's top level; a client older than that MAY find it absent, and MUST then fall back to the check's severity, which is what that binary's own gate used.
     */
    severity: "error" | "warn" | "info";
    /**
     * How long a corpus-state finding has held. Present only on the checks that measure it; absent is the normal case and means the finding has no age, not that it is new.
     */
    age?: {
      /**
       * Full sha of the commit at which the condition began, so a consumer can `git show` it.
       */
      first_commit: string;
      /**
       * The `YYYY-MM-DD` of that commit — the same day the text report prints.
       */
      first_day: string;
      /**
       * Corpus-touching commits it has held for, counting the one at HEAD. Commits rather than days, deliberately: a day count is a function of when the report was run, so the same corpus answers differently tomorrow and no consumer can cache or pin it.
       */
      commits: number;
    };
    span?: Span;
  }[];
}
/**
 * Best-effort location. Output-only and never part of the violation's identity: a line number in the baseline's comparison would make it churn on every edit above a violation. Absent is a normal outcome — anchor at line 1.
 */
export interface Span {
  line: number;
}
export interface ADoctorCheck {
  id: CheckId;
  /**
   * `doctor` only. The question in the form a person would ask it.
   */
  question: string;
  /**
   * `doctor` only. `declined` is a check whose only open offers this repository declined under `[doctor.declined]`, each against a decision record it holds (RFC-0066): not actionable, and `--strict` does not fail on it. `warn` is actionable and a normal state to be in, and does not affect the exit code without `--strict`; `fail` is wrong now and exits nonzero; `skipped` is not answerable, almost always because the repository check already failed and there is nothing to answer about. A consumer MUST NOT read `skipped` as `ok`. Every question `doctor` asks is reported on both paths, so a `skipped` check is how the answerable set stays the same shape — one check was absent from the unanswerable path rather than skipped on it until #656, and no fixture emitted this value at all, which left renaming it here undetectable.
   */
  verdict: "ok" | "warn" | "fail" | "skipped" | "declined";
  /**
   * `doctor` only. What was actually found.
   */
  detail: string;
  /**
   * `doctor` only. The command or edit that resolves this finding. **Null on `ok` and on `skipped`** — a verdict with no finding has nothing to resolve — and null-iff-actionable is held by construction rather than by agreement: the constructors for those two verdicts take no remedy. It was NOT an invariant until #656, when the prelude and policy checks each named one alongside `verdict: ok`. The text renderer suppressed them and the JSON did not, so the same green check reached a person with nothing to do and a consumer with an action against it. Advice worth reading on a healthy check goes in `detail`, which is rendered on every line — that is where the prelude check's pointer at the one networked command now lives.
   */
  remedy: string | null;
  /**
   * `doctor` only. The upgrade offers this check raises, each with the release that first made it and where it stands (RFC-0066). Absent on a check that carries none. An offer is a convention a release made available, not a defect: `open` warns, `declined` is a decline against a held record, `unhonoured` is a decline whose record is not held and still warns, `adopted` is taken up.
   */
  offers?: {
    /**
     * The offer id — the key `[doctor.declined]` takes, which is not the check id.
     */
    id: string;
    /**
     * The release tag that first made the offer, such as `cli/v0.17.0`.
     */
    since: string;
    state: "open" | "declined" | "unhonoured" | "adopted";
    /**
     * The decision record a decline resolved to, repo-relative. Present only on `declined`.
     */
    record?: string;
  }[];
}
/**
 * A best-effort source location, declared once. Output-only wherever it appears and never part of anything's identity: a line number inside a violation's identity would make the baseline churn on every edit above it. Absent is a normal outcome — anchor at line 1.
 *
 * This interface was referenced by `YidamReportEnvelope`'s JSON-Schema
 * via the `definition` "span".
 */
export interface Span1 {
  line: number;
}
/**
 * Links from the artifact into `.yidam/`.
 */
export interface CouplingLinks {
  /**
   * Every relative markdown link read in this direction.
   */
  total: number;
  /**
   * The ones whose target is not on disk. Each is also a `lint` finding: `broken-object-link` for artifact → corpus, `broken-prose-link` for corpus → artifact. A consumer that wants the locations reads `lint`, not this count.
   */
  dead: number;
}
/**
 * Links from corpus records into the artifact.
 */
export interface CouplingLinks1 {
  /**
   * Every relative markdown link read in this direction.
   */
  total: number;
  /**
   * The ones whose target is not on disk. Each is also a `lint` finding: `broken-object-link` for artifact → corpus, `broken-prose-link` for corpus → artifact. A consumer that wants the locations reads `lint`, not this count.
   */
  dead: number;
}
/**
 * Instance nodes under `.yidam/corpus/`.
 */
export interface CouplingReach {
  /**
   * Records of this kind the corpus holds.
   */
  total: number;
  /**
   * Records the artifact links to at least once.
   */
  cited: number;
  /**
   * Records that link into the artifact at least once.
   */
  citing: number;
  /**
   * Records nothing in the artifact links to, repository-relative and sorted. Where to look, NOT a list of defects: most records have no reason to be cited by code, and a consumer MUST NOT count these as findings.
   */
  uncited: string[];
}
/**
 * Decision records under `.yidam/decisions/`.
 */
export interface CouplingReach1 {
  /**
   * Records of this kind the corpus holds.
   */
  total: number;
  /**
   * Records the artifact links to at least once.
   */
  cited: number;
  /**
   * Records that link into the artifact at least once.
   */
  citing: number;
  /**
   * Records nothing in the artifact links to, repository-relative and sorted. Where to look, NOT a list of defects: most records have no reason to be cited by code, and a consumer MUST NOT count these as findings.
   */
  uncited: string[];
}
/**
 * Catalog entries under `.yidam/catalog/`.
 */
export interface CouplingReach2 {
  /**
   * Records of this kind the corpus holds.
   */
  total: number;
  /**
   * Records the artifact links to at least once.
   */
  cited: number;
  /**
   * Records that link into the artifact at least once.
   */
  citing: number;
  /**
   * Records nothing in the artifact links to, repository-relative and sorted. Where to look, NOT a list of defects: most records have no reason to be cited by code, and a consumer MUST NOT count these as findings.
   */
  uncited: string[];
}
/**
 * Links counted in one direction across the boundary between a corpus and the artifact `[object] paths` declares.
 *
 * This interface was referenced by `YidamReportEnvelope`'s JSON-Schema
 * via the `definition` "coupling_links".
 */
export interface CouplingLinks2 {
  /**
   * Every relative markdown link read in this direction.
   */
  total: number;
  /**
   * The ones whose target is not on disk. Each is also a `lint` finding: `broken-object-link` for artifact → corpus, `broken-prose-link` for corpus → artifact. A consumer that wants the locations reads `lint`, not this count.
   */
  dead: number;
}
/**
 * How far one kind of corpus record reaches across the boundary.
 *
 * This interface was referenced by `YidamReportEnvelope`'s JSON-Schema
 * via the `definition` "coupling_reach".
 */
export interface CouplingReach3 {
  /**
   * Records of this kind the corpus holds.
   */
  total: number;
  /**
   * Records the artifact links to at least once.
   */
  cited: number;
  /**
   * Records that link into the artifact at least once.
   */
  citing: number;
  /**
   * Records nothing in the artifact links to, repository-relative and sorted. Where to look, NOT a list of defects: most records have no reason to be cited by code, and a consumer MUST NOT count these as findings.
   */
  uncited: string[];
}
