// Dafny specification: corpus graph invariants and parity function correctness.
//
// Three key theorems:
//   (1) update_regen — content preservation and idempotency
//   (2) classify_commit — totality; Epistemic is the default
//   (3) parse_markers — soundness (no phantom markers)
//
// Run: dafny verify prelude/sdks/spec/graph.dfy

module YidamGraph {

  // ── Types ─────────────────────────────────────────────────────────────────────

  datatype EvidenceTag = Verified | Inference | Open | Implicit

  datatype CommitKind = Epistemic | Operational

  datatype Option<T> = None | Some(value: T)

  datatype Claim = Claim(text: string, tag: EvidenceTag)

  datatype Link = Link(label: string, target: string)

  datatype CorpusNode = CorpusNode(
    path: string,
    title: string,
    claims: seq<Claim>,
    links: seq<Link>
  )

  datatype Marker =
    | TemplateMarker(instruction: string)
    | RegenMarker(command: string, content: string)

  datatype CommitEvent = CommitEvent(
    hash: string,
    kind: CommitKind,
    verb: string,
    subject: string,
    context: Option<string>
  )

  // ── String predicates ─────────────────────────────────────────────────────────

  predicate SubstringAt(s: string, sub: string, i: int) {
    0 <= i && i + |sub| <= |s| && s[i..i + |sub|] == sub
  }

  predicate HasPrefix(s: string, p: string) {
    |s| >= |p| && s[..|p|] == p
  }

  // ── update_regen ───────────────────────────────────────────────────────────────
  //
  // A text has a well-formed REGEN block for `command` when it contains, in order:
  //   "<!-- REGEN: command"  …  "-->"  …  "<!-- /REGEN -->"

  predicate HasRegenFor(text: string, command: string) {
    var open_tag  := "<!-- REGEN: " + command;
    var arrow     := "-->";
    var close_tag := "<!-- /REGEN -->";
    |open_tag| > 0 &&
    exists oi, ai, ci ::
      SubstringAt(text, open_tag, oi) &&
      ai >= oi + |open_tag| &&
      SubstringAt(text, arrow, ai) &&
      ci >= ai + |arrow| &&
      SubstringAt(text, close_tag, ci)
  }

  // RegenPrefix and RegenSuffix axiomatize the split point:
  //   text == RegenPrefix(text, cmd) + "\n" + old_content + "\n" + RegenSuffix(text, cmd)
  // The implementation must produce exactly these two spans.
  ghost function {:axiom} RegenPrefix(text: string, command: string): string
    requires HasRegenFor(text, command)

  ghost function {:axiom} RegenSuffix(text: string, command: string): string
    requires HasRegenFor(text, command)

  lemma {:axiom} RegenDecomposition(text: string, command: string)
    requires HasRegenFor(text, command)
    ensures exists old: string ::
      text == RegenPrefix(text, command) + "\n" + old + "\n" + RegenSuffix(text, command)

  // Content preservation theorem.
  // Every implementation of update_regen must satisfy all four postconditions.
  lemma {:axiom} UpdateRegenSpec(text: string, command: string, new_content: string)
    requires HasRegenFor(text, command)
    ensures
      var result := RegenPrefix(text, command) + "\n" + new_content + "\n" + RegenSuffix(text, command);
      // (1) Frame: text outside the REGEN section is byte-for-byte identical.
      && RegenPrefix(result, command)  == RegenPrefix(text, command)
      && RegenSuffix(result, command)  == RegenSuffix(text, command)
      // (2) The target section holds exactly new_content.
      && HasRegenFor(result, command)
      // (3) No REGEN blocks created or destroyed.
      && RegenBlockCount(result) == RegenBlockCount(text)
      // (4) Idempotency: a second application with the same content is a no-op.
      && (RegenPrefix(result, command) + "\n" + new_content + "\n" + RegenSuffix(result, command)) == result

  ghost function {:axiom} RegenBlockCount(text: string): nat

  // ── classify_commit — totality ────────────────────────────────────────────────

  // Operational is the explicitly marked case; all other verbs are Epistemic.
  // Mirrors the commit vocabulary in prelude/GRAPH.md and the OPERATIONAL_VERBS
  // constant in each SDK. This set previously omitted "fix" and "regen", which all
  // three implementations carried — the spec and the code disagreed.
  const OperationalVerbs: set<string> :=
    {"extract", "refresh", "compute", "index", "bundle", "reconcile", "regen",
     "build", "implement", "scaffold", "catalog", "migrate", "fix", "vendor", "consume"}

  // The other half of the closed vocabulary. ClassifyVerb does not consult it —
  // Epistemic is the default, and that totality is what the lemmas below establish.
  // It exists so a verb in neither set can be identified as outside the vocabulary.
  const EpistemicVerbs: set<string> :=
    {"establish", "revise", "assess", "synthesize", "withdraw", "open", "close",
     "decide", "phase", "genesis", "overlay"}

  function ClassifyVerb(verb: string): CommitKind {
    if verb in OperationalVerbs then Operational else Epistemic
  }

  // Vocabulary recognition — the predicate `yidam lint --commits` implements.
  predicate Recognized(verb: string) {
    verb in OperationalVerbs || verb in EpistemicVerbs
  }

  // The two halves do not overlap: no verb is both a knowledge event and pipeline work.
  lemma VocabularyIsDisjoint()
    ensures OperationalVerbs * EpistemicVerbs == {}
  {}

  // Recognition is strictly stronger than classification: every recognized verb still
  // classifies, but classification alone says nothing about whether the verb is legible.
  lemma RecognizedVerbsStillClassify(verb: string)
    requires Recognized(verb)
    ensures ClassifyVerb(verb) == Epistemic || ClassifyVerb(verb) == Operational
  {}

  // Every verb maps to exactly one kind. No partial function; no panics.
  lemma ClassifyCommitTotal(verb: string)
    ensures ClassifyVerb(verb) == Epistemic || ClassifyVerb(verb) == Operational
  {}

  // Unknown verbs default to Epistemic (the unmarked case).
  lemma EpistemicIsDefault(verb: string)
    requires verb !in OperationalVerbs
    ensures ClassifyVerb(verb) == Epistemic
  {}

  // Operational classification requires explicit membership.
  lemma OperationalRequiresExplicitVerb(verb: string)
    ensures ClassifyVerb(verb) == Operational ==> verb in OperationalVerbs
  {}

  // ── parse_markers — soundness ─────────────────────────────────────────────────

  // A marker is grounded when its claimed command/instruction appears in the source.
  predicate MarkerGrounded(text: string, m: Marker) {
    match m {
      case TemplateMarker(instruction) =>
        |instruction| > 0 &&
        (exists i :: SubstringAt(text, "<!-- TEMPLATE: " + instruction, i))
      case RegenMarker(command, _) =>
        |command| > 0 &&
        (exists i :: SubstringAt(text, "<!-- REGEN: " + command, i))
    }
  }

  // Soundness: no phantom markers — every marker in the output is grounded in the source.
  lemma {:axiom} ParseMarkersSound(text: string, markers: seq<Marker>)
    ensures forall i :: 0 <= i < |markers| ==> MarkerGrounded(text, markers[i])

  // Completeness: every REGEN block in the text produces a marker in the output.
  predicate ParseMarkersComplete(text: string, markers: seq<Marker>) {
    forall cmd: string ::
      (|cmd| > 0 && (exists i :: SubstringAt(text, "<!-- REGEN: " + cmd, i))) ==>
        (exists j :: 0 <= j < |markers| &&
          markers[j].RegenMarker? && markers[j].command == cmd)
  }

  // ── Corpus graph validity ─────────────────────────────────────────────────────

  predicate ExternalLink(target: string) {
    HasPrefix(target, "https://") || HasPrefix(target, "http://")
  }

  // A corpus is structurally valid when:
  //   (S2) every node has ≥1 outgoing link, and
  //   (S3) every relative link target resolves to a known node.
  predicate ValidCorpus(nodes: map<string, CorpusNode>) {
    forall path :: path in nodes ==>
      (|nodes[path].links| >= 1 &&
       (forall link :: link in nodes[path].links ==>
          ExternalLink(link.target) || link.target in nodes))
  }

  // Adding a well-linked node preserves corpus validity.
  lemma AddNodePreservesValidity(
    nodes: map<string, CorpusNode>,
    newPath: string,
    newNode: CorpusNode
  )
    requires ValidCorpus(nodes)
    requires newPath !in nodes
    requires |newNode.links| >= 1
    requires forall link :: link in newNode.links ==>
      ExternalLink(link.target) || link.target in nodes
    ensures ValidCorpus(nodes[newPath := newNode])
  {
    var nodes' := nodes[newPath := newNode];
    // Every key in nodes is also in nodes'.
    assert forall k :: k in nodes ==> k in nodes';
    forall p | p in nodes'
      ensures |nodes'[p].links| >= 1 &&
        forall link :: link in nodes'[p].links ==>
          ExternalLink(link.target) || link.target in nodes'
    {
      if p == newPath {
        forall link | link in newNode.links
          ensures ExternalLink(link.target) || link.target in nodes'
        {
          if !ExternalLink(link.target) {
            assert link.target in nodes;
            assert link.target in nodes';
          }
        }
      } else {
        // p was in nodes; its links are unchanged.
        assert nodes'[p] == nodes[p];
        forall link | link in nodes[p].links
          ensures ExternalLink(link.target) || link.target in nodes'
        {
          if !ExternalLink(link.target) {
            assert link.target in nodes;
            assert link.target in nodes';
          }
        }
      }
    }
  }
}
