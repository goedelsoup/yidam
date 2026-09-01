// Dafny specification: corpus graph invariants and parity function correctness.
//
// Three key claims, and what `dafny verify` establishes about each:
//   (1) update_regen — content preservation and idempotency   PROVED (UpdateRegenSpec)
//   (2) classify_commit — totality; Epistemic is the default  PROVED
//   (3) parse_markers — soundness (no phantom markers)        PROVED (ParseMarkersSound)
//
// (1) and (3) were `{:axiom}` — obligations *on the implementation*, which Dafny could not
// see, counted toward the "verified" total exactly as a proof is. #499 discharged both by
// modelling the two functions here and proving the claims about the model. The model is
// `markers.rs` transcribed: `UpdateRegen` finds the same three offsets `update_regen` finds,
// `ParseFrom` walks lines the way `parse_markers` walks them.
//
// Modelling them found three claims **false as they were written** — two of them assumed by
// an axiom, which is what an axiom conceals, and one a predicate that stated a property and
// was then used by nothing at all. Each is now a proved refutation:
//   - `UpdateRegenSpec` claimed REGEN-block counts are preserved unconditionally. They are
//     not; `RegenBlockCountWasTheWrongInstrument` proves it and says what replaced the clause.
//   - `MarkerGrounded` (here `GroundedBySubstring`) claimed a marker's command appears in the
//     source as a raw substring after "<!-- REGEN: ". It does not — `parse_markers` trims, and
//     one extra space in the tag is enough. `TheSubstringFormOfGroundingIsFalse`.
//   - `ParseMarkersComplete` claimed every REGEN block yields a marker. An unterminated block
//     swallows every one below it: `ParseMarkersIsNotComplete`, filed as #524.
//
// What is *not* modelled, stated so nobody reads more into the green than is there: Dafny
// still cannot see the Rust. These lemmas prove the model correct, and `parity.rs` checks the
// three runtimes against each other; nothing mechanically checks the model against
// `markers.rs`. The model is short and transcribed line by line so that a reader can.
//
// Run: mise run verify   (or `dafny verify graph.dfy` from this directory)

module YidamGraph {

  // ── Types ─────────────────────────────────────────────────────────────────────

  datatype EvidenceTag = Verified | Inference | Open | Implicit

  datatype CommitKind = Epistemic | Operational

  datatype Option<T> = None | Some(value: T)

  datatype Claim = Claim(text: string, tag: EvidenceTag)

  // `label` is a Dafny keyword, so the field the other three models spell `label` is
  // `linkLabel` here. Renaming it is what lets this file parse at all — it never has.
  datatype Link = Link(linkLabel: string, target: string)

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

  predicate HasSuffix(s: string, p: string) {
    |s| >= |p| && s[|s| - |p|..] == p
  }

  predicate ContainsNo(s: string, sub: string) {
    forall i :: 0 <= i <= |s| ==> !SubstringAt(s, sub, i)
  }

  // `str::find` and `str::trim`, which is what both functions below are built out of.
  //
  // Rust's `trim` is Unicode-aware and this is not: `IsSpace` is the ASCII four. Every
  // character the marker syntax uses is ASCII, so the two agree on every input the parser is
  // pointed at — but that is an assumption about inputs, not a proof, and it is the one place
  // the model is deliberately narrower than the code.
  predicate IsSpace(c: char) { c == ' ' || c == '\t' || c == '\r' || c == '\n' }

  function TrimLeft(s: string): string
    decreases |s|
  { if |s| > 0 && IsSpace(s[0]) then TrimLeft(s[1..]) else s }

  function TrimRight(s: string): string
    decreases |s|
  { if |s| > 0 && IsSpace(s[|s| - 1]) then TrimRight(s[..|s| - 1]) else s }

  function Trim(s: string): string { TrimLeft(TrimRight(s)) }

  // The first occurrence of `sub` at or after `from` — `str::find` on a suffix.
  //
  // The three postconditions are what every proof below runs on: where it landed, that it
  // landed on the *first* match, and — when it found nothing — that there is nothing to find.
  function FindFrom(s: string, sub: string, from: nat): Option<nat>
    requires from <= |s|
    decreases |s| - from
    ensures FindFrom(s, sub, from).Some? ==>
      from <= FindFrom(s, sub, from).value &&
      SubstringAt(s, sub, FindFrom(s, sub, from).value)
    ensures FindFrom(s, sub, from).Some? ==>
      forall j :: from <= j < FindFrom(s, sub, from).value ==> !SubstringAt(s, sub, j)
    ensures FindFrom(s, sub, from).None? ==>
      forall j :: from <= j <= |s| ==> !SubstringAt(s, sub, j)
  {
    if from + |sub| > |s| then None
    else if s[from..from + |sub|] == sub then Some(from)
    else FindFrom(s, sub, from + 1)
  }

  // Dafny will not always reduce a slice of a string literal to another literal — it manages
  // some and not others, with no pattern worth guessing at. The witness lemmas below prove any
  // of them the same way instead: character by character, which the solver does always
  // discharge on a literal.
  lemma SliceIsLiteral(s: string, a: nat, b: nat, lit: string)
    requires a <= b <= |s| && b - a == |lit|
    requires forall i :: 0 <= i < |lit| ==> s[a + i] == lit[i]
    ensures s[a..b] == lit
  {
    assert forall i {:trigger s[a..b][i]} :: 0 <= i < |lit| ==> s[a..b][i] == s[a + i];
  }

  lemma SliceOfSharedPrefix(s: string, t: string, a: nat, b: nat, cut: nat)
    requires a <= b <= cut <= |s| && cut <= |t|
    requires s[..cut] == t[..cut]
    ensures s[a..b] == t[a..b]
  {
    assert s[a..b] == s[..cut][a..b];
    assert t[a..b] == t[..cut][a..b];
  }

  // A search that terminates inside a shared prefix finds the same thing in both strings.
  // This is what carries "the open tag and the arrow are where they were" across the rewrite.
  lemma FindAgreesOnSharedPrefix(s: string, t: string, sub: string, from: nat, cut: nat)
    requires from <= cut <= |s| && cut <= |t|
    requires s[..cut] == t[..cut]
    requires FindFrom(s, sub, from).Some?
    requires FindFrom(s, sub, from).value + |sub| <= cut
    ensures FindFrom(t, sub, from) == FindFrom(s, sub, from)
    decreases cut - from
  {
    var i := FindFrom(s, sub, from).value;
    SliceOfSharedPrefix(s, t, from, from + |sub|, cut);
    if from < i {
      assert !SubstringAt(s, sub, from);
      FindAgreesOnSharedPrefix(s, t, sub, from + 1, cut);
    }
  }

  // A stretch with no occurrence in it is skipped over.
  lemma FindSkips(s: string, sub: string, from: nat, upto: nat)
    requires from <= upto <= |s|
    requires forall j :: from <= j < upto ==> !SubstringAt(s, sub, j)
    ensures FindFrom(s, sub, from) == FindFrom(s, sub, upto)
    decreases upto - from
  {
    if from < upto {
      FindSkips(s, sub, from + 1, upto);
    }
  }

  // No occurrence of a newline-free needle straddles a newline. The body `update_regen`
  // writes is wrapped in newlines, and neither tag contains one, so this is what keeps a
  // re-scan of the result from finding a close tag early.
  lemma NoOccurrenceAcrossNewline(s: string, sub: string, j: nat, k: nat)
    requires 0 <= j <= k < |s| && k < j + |sub|
    requires s[k] == '\n'
    requires forall i :: 0 <= i < |sub| ==> sub[i] != '\n'
    ensures !SubstringAt(s, sub, j)
  {
    if SubstringAt(s, sub, j) {
      assert s[j..j + |sub|][k - j] == sub[k - j];
      assert s[k] == sub[k - j];
    }
  }

  // ── update_regen ───────────────────────────────────────────────────────────────
  //
  // `update_regen(text, command, new_content)` finds, in order:
  //   "<!-- REGEN: command"  …  "-->"  …  "<!-- /REGEN -->"
  // and replaces what lies between the arrow and the close tag. Each of the three is the
  // *first* match at or after where the previous one ended, which is what `RegenSpan` says.

  const RegenOpen:  string := "<!-- REGEN: "
  const RegenArrow: string := "-->"
  const RegenClose: string := "<!-- /REGEN -->"

  lemma TagShapes()
    ensures forall i :: 0 <= i < |RegenClose| ==> RegenClose[i] != '\n'
    ensures forall i :: 0 <= i < |RegenOpen|  ==> RegenOpen[i]  != '\n'
  {}

  // The four offsets, in the order the implementation computes them.
  datatype Span = Span(open: nat, arrow: nat, body: nat, close: nat)

  // A text has a well-formed REGEN block for `command` when the three markers occur in order.
  // This is the declarative statement the file has always carried; `RegenSpanFindsEveryBlock`
  // below is what ties it to the greedy left-to-right search the implementation performs.
  ghost predicate HasRegenFor(text: string, command: string) {
    exists oi, ai, ci ::
      SubstringAt(text, RegenOpen + command, oi) &&
      ai >= oi + |RegenOpen + command| &&
      SubstringAt(text, RegenArrow, ai) &&
      ci >= ai + |RegenArrow| &&
      SubstringAt(text, RegenClose, ci)
  }

  function RegenSpan(text: string, command: string): Option<Span>
    ensures RegenSpan(text, command).Some? ==>
      var sp := RegenSpan(text, command).value;
      && FindFrom(text, RegenOpen + command, 0) == Some(sp.open)
      && sp.open + |RegenOpen + command| <= sp.arrow
      && FindFrom(text, RegenArrow, sp.open + |RegenOpen + command|) == Some(sp.arrow)
      && sp.body == sp.arrow + |RegenArrow|
      && sp.body <= sp.close <= |text|
      && FindFrom(text, RegenClose, sp.body) == Some(sp.close)
      && SubstringAt(text, RegenClose, sp.close)
      && SubstringAt(text, RegenArrow, sp.arrow)
  {
    match FindFrom(text, RegenOpen + command, 0)
      case None => None
      case Some(op) =>
        var afterOpen := op + |RegenOpen + command|;
        match FindFrom(text, RegenArrow, afterOpen)
          case None => None
          case Some(ai) =>
            var bs := ai + |RegenArrow|;
            match FindFrom(text, RegenClose, bs)
              case None => None
              case Some(ci) => Some(Span(op, ai, bs, ci))
  }

  // The greedy search misses no well-formed block, and invents none.
  //
  // Not obvious in the ⇒ direction, and worth stating because the two halves are written
  // differently on purpose: `HasRegenFor` is three unconstrained existentials, and the
  // implementation takes the *first* match at every step. Taking the first can only move each
  // offset left, which leaves room for the next — so a block that exists anywhere is a block
  // the scan finds.
  lemma RegenSpanFindsEveryBlock(text: string, command: string)
    ensures HasRegenFor(text, command) <==> RegenSpan(text, command).Some?
  {
    if HasRegenFor(text, command) {
      var oi, ai, ci :| SubstringAt(text, RegenOpen + command, oi) &&
                        ai >= oi + |RegenOpen + command| &&
                        SubstringAt(text, RegenArrow, ai) &&
                        ci >= ai + |RegenArrow| &&
                        SubstringAt(text, RegenClose, ci);
      assert FindFrom(text, RegenOpen + command, 0).Some?;
      var op := FindFrom(text, RegenOpen + command, 0).value;
      assert op <= oi;
      var afterOpen := op + |RegenOpen + command|;
      assert afterOpen <= ai;
      assert FindFrom(text, RegenArrow, afterOpen).Some?;
      var ar := FindFrom(text, RegenArrow, afterOpen).value;
      assert ar <= ai;
      assert ar + |RegenArrow| <= ci;
      assert FindFrom(text, RegenClose, ar + |RegenArrow|).Some?;
    }
    if RegenSpan(text, command).Some? {
      var sp := RegenSpan(text, command).value;
      assert SubstringAt(text, RegenOpen + command, sp.open);
    }
  }

  // The body the implementation writes. The empty case is not a special case for tidiness —
  // `update_regen` collapses it so that clearing a section does not leave a blank line
  // between the markers, and a model that wrote "\n\n" would not be a model of it.
  function RegenBody(newContent: string): string {
    if newContent == "" then "\n" else "\n" + newContent + "\n"
  }

  function UpdateRegen(text: string, command: string, newContent: string): string
  {
    match RegenSpan(text, command)
      case None => text
      case Some(sp) => text[..sp.body] + RegenBody(newContent) + text[sp.close..]
  }

  // Nothing in the freshly written body matches the close tag, provided the caller did not
  // write one into `newContent` — which is the precondition the spec below carries, and which
  // the axiom it replaces did not.
  lemma CloseTagNotInBody(result: string, nc: string, cs: nat)
    requires cs + |RegenBody(nc)| <= |result|
    requires result[cs..cs + |RegenBody(nc)|] == RegenBody(nc)
    requires ContainsNo(nc, RegenClose)
    ensures forall j :: cs <= j < cs + |RegenBody(nc)| ==> !SubstringAt(result, RegenClose, j)
  {
    TagShapes();
    var body := RegenBody(nc);
    assert result[cs] == '\n' by { assert result[cs..cs + |body|][0] == body[0]; }
    assert result[cs + |body| - 1] == '\n' by {
      assert result[cs..cs + |body|][|body| - 1] == body[|body| - 1];
    }
    forall j | cs <= j < cs + |body|
      ensures !SubstringAt(result, RegenClose, j)
    {
      if j == cs {
        NoOccurrenceAcrossNewline(result, RegenClose, j, cs);
      } else if j + |RegenClose| > cs + |body| - 1 {
        NoOccurrenceAcrossNewline(result, RegenClose, j, cs + |body| - 1);
      } else {
        // Strictly inside `nc`, which the precondition says holds no close tag.
        assert nc != "";
        var k := j - cs - 1;
        if SubstringAt(result, RegenClose, j) {
          forall m | 0 <= m < |nc|
            ensures result[cs + 1 + m] == nc[m]
          {
            assert result[cs..cs + |body|][1 + m] == body[1 + m];
          }
          assert forall i {:trigger nc[k..k + |RegenClose|][i]} :: 0 <= i < |RegenClose| ==>
            nc[k..k + |RegenClose|][i] == result[j..j + |RegenClose|][i];
          assert nc[k..k + |RegenClose|] == RegenClose;
          assert SubstringAt(nc, RegenClose, k);
        }
      }
    }
  }

  // **Claim (1).** Content preservation and idempotency, as a lemma with a body.
  //
  // Three of the axiom's four clauses survive as written. The fourth — "(3) No REGEN blocks
  // created or destroyed", stated over an axiomatized `RegenBlockCount` — does not; see
  // `RegenBlockCountWasTheWrongInstrument`.
  //
  // `ContainsNo(newContent, RegenClose)` is new, and it is the price of the axiom being an
  // axiom: a caller who writes a close tag into the new content terminates the section early,
  // and every clause below is false of that call. Nothing said so before.
  lemma UpdateRegenSpec(text: string, command: string, newContent: string)
    requires HasRegenFor(text, command)
    requires ContainsNo(newContent, RegenClose)
    ensures RegenSpan(text, command).Some?
    ensures
      var sp     := RegenSpan(text, command).value;
      var result := UpdateRegen(text, command, newContent);
      var body   := RegenBody(newContent);
      // (1) Frame: the text before the body and from the close tag on is byte-for-byte equal.
      && result[..sp.body] == text[..sp.body]
      && result[sp.body + |body|..] == text[sp.close..]
      // (2) The section holds exactly newContent, and the block is still a block: the same
      //     open tag, the same arrow, the same body start, and a close tag right after it.
      && result[sp.body..sp.body + |body|] == body
      && RegenSpan(result, command) == Some(Span(sp.open, sp.arrow, sp.body, sp.body + |body|))
      && HasRegenFor(result, command)
      // (4) Idempotency: a second application with the same content changes nothing.
      && UpdateRegen(result, command, newContent) == result
  {
    RegenSpanFindsEveryBlock(text, command);
    TagShapes();
    var sp     := RegenSpan(text, command).value;
    var body   := RegenBody(newContent);
    var result := UpdateRegen(text, command, newContent);
    assert result == text[..sp.body] + body + text[sp.close..];

    assert result[..sp.body] == text[..sp.body];
    assert result[sp.body..sp.body + |body|] == body;
    assert result[sp.body + |body|..] == text[sp.close..];

    // The open tag and the arrow are inside the shared prefix, so both finds land where they
    // landed in `text`.
    FindAgreesOnSharedPrefix(text, result, RegenOpen + command, 0, sp.body);
    FindAgreesOnSharedPrefix(text, result, RegenArrow, sp.open + |RegenOpen + command|, sp.body);

    // The close tag: nothing in the body matches, and the close tag `text` already had is the
    // first thing after it.
    CloseTagNotInBody(result, newContent, sp.body);
    FindSkips(result, RegenClose, sp.body, sp.body + |body|);
    assert SubstringAt(result, RegenClose, sp.body + |body|) by {
      assert result[sp.body + |body|..][..|RegenClose|] == text[sp.close..][..|RegenClose|];
      assert text[sp.close..][..|RegenClose|] == text[sp.close..sp.close + |RegenClose|];
    }
    assert FindFrom(result, RegenClose, sp.body + |body|) == Some(sp.body + |body|);
    assert RegenSpan(result, command)
        == Some(Span(sp.open, sp.arrow, sp.body, sp.body + |body|));
    RegenSpanFindsEveryBlock(result, command);

    // Idempotency falls out of (2): the second run splits at the same body start and the new
    // close, and re-lays the identical three pieces.
    assert UpdateRegen(result, command, newContent)
        == result[..sp.body] + body + result[sp.body + |body|..];
  }

  // The spec above is about something. A lemma whose preconditions no input satisfies proves
  // its postcondition of nothing at all, and reads exactly like one that does.
  lemma UpdateRegenSpecIsNotVacuous()
    ensures var text := "a\n<!-- REGEN: due -->\nold\n<!-- /REGEN -->\nz";
            HasRegenFor(text, "due") && ContainsNo("new", RegenClose)
  {
    var text := "a\n<!-- REGEN: due -->\nold\n<!-- /REGEN -->\nz";
    assert RegenOpen + "due" == "<!-- REGEN: due";
    SliceIsLiteral(text, 2, 17, "<!-- REGEN: due");
    SliceIsLiteral(text, 18, 21, "-->");
    SliceIsLiteral(text, 26, 41, "<!-- /REGEN -->");
    assert SubstringAt(text, RegenOpen + "due", 2);
    assert SubstringAt(text, RegenArrow, 18);
    assert SubstringAt(text, RegenClose, 26);
    assert HasRegenFor(text, "due");
    forall i | 0 <= i <= |"new"| ensures !SubstringAt("new", RegenClose, i) {}
  }

  // `update_regen` collapses the empty body: clearing a section leaves the two markers on
  // consecutive lines rather than with a blank line between them. Nothing above pins that —
  // the spec is written in terms of `RegenBody`, so a model that wrote "\n\n" would satisfy
  // every clause of it. This is what fails if `RegenBody` stops special-casing the empty
  // string, and it is the only thing that does.
  lemma ClearingASectionLeavesNoBlankLine(text: string, command: string)
    requires RegenSpan(text, command).Some?
    ensures var sp := RegenSpan(text, command).value;
            && UpdateRegen(text, command, "") == text[..sp.body] + "\n" + text[sp.close..]
            && UpdateRegen(text, command, "") != text[..sp.body] + "\n\n" + text[sp.close..]
  {
    var sp := RegenSpan(text, command).value;
    assert |text[..sp.body] + "\n" + text[sp.close..]|
        != |text[..sp.body] + "\n\n" + text[sp.close..]|;
  }

  // Clause (3) of the axiom — "No REGEN blocks created or destroyed", over an axiomatized
  // `RegenBlockCount` — is not restated above, and this says why rather than leaving its
  // absence to be noticed.
  //
  // It was false as written: a caller passing "<!-- REGEN: other -->" as `new_content` raises
  // the count by one, and nothing in the axiom forbade it. It was also the weaker instrument.
  // Clause (1) gives byte-for-byte equality of everything outside the section, which says
  // more about those blocks than any count of them could; clause (2) gives the edited block
  // back intact. What is left over is the body, and the body is the caller's string. There is
  // no statement about block counts that `update_regen` is answerable for.
  //
  // The lemma is the first half of that: a block appears that the text did not have.
  lemma RegenBlockCountWasTheWrongInstrument()
    ensures var text := "<!-- REGEN: a -->\n\n<!-- /REGEN -->";
            var nc   := "<!-- REGEN: b -->";
            && HasRegenFor(text, "a")
            && ContainsNo(nc, RegenClose)
            && !HasRegenFor(text, "b")
            && HasRegenFor(UpdateRegen(text, "a", nc), "b")
  {
    var text := "<!-- REGEN: a -->\n\n<!-- /REGEN -->";
    var nc   := "<!-- REGEN: b -->";
    assert RegenOpen + "a" == "<!-- REGEN: a";
    SliceIsLiteral(text, 0, 13, "<!-- REGEN: a");
    SliceIsLiteral(text, 14, 17, "-->");
    SliceIsLiteral(text, 19, 34, "<!-- /REGEN -->");
    assert SubstringAt(text, RegenOpen + "a", 0);
    assert SubstringAt(text, RegenArrow, 14);
    assert SubstringAt(text, RegenClose, 19);
    assert HasRegenFor(text, "a");
    // `nc` holds no '/', and the close tag is '/' at offset 5.
    assert RegenClose[5] == '/';
    forall i | 0 <= i <= |nc| ensures !SubstringAt(nc, RegenClose, i) {
      if SubstringAt(nc, RegenClose, i) { assert nc[i..i + |RegenClose|][5] == nc[i + 5]; }
    }
    assert !HasRegenFor(text, "b") by {
      forall i | 0 <= i <= |text| ensures !SubstringAt(text, RegenOpen + "b", i) {
        assert (RegenOpen + "b")[12] == 'b';
        if SubstringAt(text, RegenOpen + "b", i) {
          assert text[i..i + 13][12] == text[i + 12];
        }
      }
    }
    UpdateRegenSpec(text, "a", nc);
    var sp := RegenSpan(text, "a").value;
    var result := UpdateRegen(text, "a", nc);
    assert result[sp.body..sp.body + |RegenBody(nc)|] == RegenBody(nc);
    assert result[sp.body + 1..sp.body + 1 + |nc|] == nc by {
      assert forall i {:trigger RegenBody(nc)[1 + i]} :: 0 <= i < |nc| ==>
        result[sp.body + 1 + i] == nc[i];
      SliceIsLiteral(result, sp.body + 1, sp.body + 1 + |nc|, nc);
    }
    assert SubstringAt(result, RegenOpen + "b", sp.body + 1) by {
      SliceIsLiteral(nc, 0, 13, "<!-- REGEN: b");
      SliceIsLiteral(result, sp.body + 1, sp.body + 1 + 13, "<!-- REGEN: b");
    }
    assert SubstringAt(result, RegenArrow, sp.body + 1 + 14) by {
      SliceIsLiteral(nc, 14, 17, "-->");
      SliceIsLiteral(result, sp.body + 15, sp.body + 18, "-->");
    }
    assert SubstringAt(result, RegenClose, sp.body + |RegenBody(nc)|);
    assert HasRegenFor(result, "b");
  }
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
    {"establish", "revise", "assess", "scope", "synthesize", "withdraw", "open", "close",
     "transport", "resolve", "adopt", "decide", "phase", "genesis", "overlay"}

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
  //
  // `parse_markers` is a line scan, so the model is one. It walks a `seq<string>` with an
  // index the way the implementation walks a `Lines` iterator, and the three functions that
  // decide what a line means — `IsTemplateLine`, `RegenOpenClosesOnItsLine`, `RegenCommand` —
  // are the `strip_prefix`/`strip_suffix`/`trim` chain of `markers.rs`, transcribed.

  const TemplateTag: string := "<!-- TEMPLATE:"
  const RegenTag:    string := "<!-- REGEN:"
  const CloseLine:   string := "<!-- /REGEN -->"

  // `str::lines`: split on '\n', no trailing empty segment. A '\r' left on the end of a line
  // is removed by `Trim` at every point the model inspects one, as it is in the Rust.
  function Lines(text: string): seq<string>
    decreases |text|
  {
    if |text| == 0 then []
    else match FindFrom(text, "\n", 0)
      case None => [text]
      case Some(i) => [text[..i]] + Lines(text[i + 1..])
  }

  // ── What a line has to look like to open a marker ─────────────────────────────

  predicate IsTemplateLine(t: string) {
    HasPrefix(t, TemplateTag) && HasSuffix(t[|TemplateTag|..], RegenArrow)
  }

  function TemplateInstruction(t: string): string
    requires IsTemplateLine(t)
  { var rest := t[|TemplateTag|..]; Trim(rest[..|rest| - |RegenArrow|]) }

  predicate RegenOpenClosesOnItsLine(t: string)
    requires HasPrefix(t, RegenTag)
  { HasSuffix(TrimRight(t[|RegenTag|..]), RegenArrow) }

  function RegenCommand(t: string): string
    requires HasPrefix(t, RegenTag)
  {
    var rest := t[|RegenTag|..];
    var r := TrimRight(rest);
    if HasSuffix(r, RegenArrow) then Trim(r[..|r| - |RegenArrow|]) else Trim(rest)
  }

  // A line *opens* a marker when the parser, reading that line, would emit exactly it. This
  // is grounding: a marker that no line opens is a marker the parser invented.
  predicate Opens(line: string, m: Marker) {
    var t := Trim(line);
    match m
      case TemplateMarker(instruction) =>
        IsTemplateLine(t) && instruction == TemplateInstruction(t)
      case RegenMarker(command, _) =>
        HasPrefix(t, RegenTag) && command == RegenCommand(t)
  }

  // ── The scan ──────────────────────────────────────────────────────────────────

  // `for inner in lines.by_ref() { if t.ends_with("-->") { break } }` — the multi-line open
  // tag's arrow, consumed along with the line it is on.
  function SkipToArrow(lines: seq<string>, i: nat): nat
    requires i <= |lines|
    decreases |lines| - i
    ensures i <= SkipToArrow(lines, i) <= |lines|
  {
    if i == |lines| then i
    else if HasSuffix(Trim(lines[i]), RegenArrow) then i + 1
    else SkipToArrow(lines, i + 1)
  }

  function SkipToClose(lines: seq<string>, i: nat): nat
    requires i <= |lines|
    decreases |lines| - i
    ensures i <= SkipToClose(lines, i) <= |lines|
  {
    if i == |lines| then i
    else if Trim(lines[i]) == CloseLine then i + 1
    else SkipToClose(lines, i + 1)
  }

  function Join(ls: seq<string>, sep: string): string
    decreases |ls|
  { if |ls| == 0 then "" else if |ls| == 1 then ls[0] else ls[0] + sep + Join(ls[1..], sep) }

  function Content(lines: seq<string>, j: nat, k: nat): string
    requires j <= k <= |lines|
  {
    var body := if k > j && Trim(lines[k - 1]) == CloseLine then lines[j..k - 1] else lines[j..k];
    Trim(Join(body, "\n"))
  }

  // **Claim (3).** The soundness postcondition is carried on the scan itself, so every call
  // discharges it: `ParseFrom` cannot return a marker that no line in the range it read
  // opens. The recursion is the implementation's loop; the `k` the REGEN branch resumes from
  // is where `lines.by_ref()` left the iterator.
  function ParseFrom(lines: seq<string>, i: nat): seq<Marker>
    requires i <= |lines|
    decreases |lines| - i
    ensures forall m :: m in ParseFrom(lines, i) ==>
      exists k :: i <= k < |lines| && Opens(lines[k], m)
  {
    if i == |lines| then []
    else
      var t := Trim(lines[i]);
      if IsTemplateLine(t) then
        [TemplateMarker(TemplateInstruction(t))] + ParseFrom(lines, i + 1)
      else if HasPrefix(t, RegenTag) then
        var j := if RegenOpenClosesOnItsLine(t) then i + 1 else SkipToArrow(lines, i + 1);
        var k := SkipToClose(lines, j);
        [RegenMarker(RegenCommand(t), Content(lines, j, k))] + ParseFrom(lines, k)
      else ParseFrom(lines, i + 1)
  }

  function ParseMarkers(text: string): seq<Marker> { ParseFrom(Lines(text), 0) }

  // Soundness: no phantom markers. Every marker the parser returns is one that some line of
  // the source opens.
  lemma ParseMarkersSound(text: string)
    ensures forall m :: m in ParseMarkers(text) ==>
      exists k :: 0 <= k < |Lines(text)| && Opens(Lines(text)[k], m)
  {
    assert ParseMarkers(text) == ParseFrom(Lines(text), 0);
    assert forall m :: m in ParseFrom(Lines(text), 0) ==>
      exists k :: 0 <= k < |Lines(text)| && Opens(Lines(text)[k], m);
  }

  // ── What the axiom claimed, and why it is not what is proved above ────────────

  // The grounding the axiom asserted: that a marker's command appears in the source as a raw
  // substring, immediately after "<!-- REGEN: ".
  ghost predicate GroundedBySubstring(text: string, m: Marker) {
    match m {
      case TemplateMarker(instruction) =>
        |instruction| > 0 &&
        (exists i :: SubstringAt(text, "<!-- TEMPLATE: " + instruction, i))
      case RegenMarker(command, _) =>
        |command| > 0 &&
        (exists i :: SubstringAt(text, RegenOpen + command, i))
    }
  }

  // And it is false. `parse_markers` trims: the command it reports is the text between the
  // tag and the arrow with its whitespace removed, so a source that spells the tag with two
  // spaces produces a marker whose command appears nowhere in the form the axiom names. One
  // extra space is enough.
  //
  // This is the shape of thing an `{:axiom}` hides. Dafny counted this claim toward "13
  // verified" for as long as the file existed, and it was never true of the parser.
  lemma {:fuel TrimLeft, 6, 7} {:fuel TrimRight, 6, 7} {:fuel Trim, 6, 7}
        TheSubstringFormOfGroundingIsFalse()
    ensures var line := "<!-- REGEN:  x -->";
            && ParseFrom([line], 0) == [RegenMarker("x", "")]
            && !GroundedBySubstring(line, RegenMarker("x", ""))
  {
    var line := "<!-- REGEN:  x -->";
    assert Trim(line) == line;
    assert !IsTemplateLine(line) by {
      if HasPrefix(line, TemplateTag) { assert line[..|TemplateTag|][5] == TemplateTag[5]; }
    }
    assert HasPrefix(line, RegenTag);
    assert line[|RegenTag|..] == "  x -->";
    assert TrimRight("  x -->") == "  x -->";
    assert RegenOpenClosesOnItsLine(line);
    assert "  x -->"[..|"  x -->"| - |RegenArrow|] == "  x ";
    assert Trim("  x ") == "x";
    assert RegenCommand(line) == "x";
    assert SkipToClose([line], 1) == 1;
    assert Content([line], 1, 1) == "";
    assert ParseFrom([line], 1) == [];
    assert ParseFrom([line], 0) == [RegenMarker("x", "")];
    forall i | 0 <= i <= |line| - |RegenOpen + "x"|
      ensures !SubstringAt(line, RegenOpen + "x", i)
    {
      assert line[i..i + 13][0]  == line[i];
      assert line[i..i + 13][12] == line[i + 12];
    }
  }

  // Soundness is one-sided: it says every marker came from a line, and nothing about which
  // lines a marker consumed. Mutating `SkipToArrow` to stop one line short leaves it green —
  // the marker is still grounded, its content is merely wrong. So the multi-line open tag,
  // which is the only path with a boundary to get wrong, is pinned by a witness instead.
  //
  // Five lines, one block, and the command is on a different line from the arrow that closes
  // its tag. `parse_markers` consumes the arrow line before it starts collecting content;
  // stopping either side of it changes the content this asserts.
  lemma {:fuel TrimLeft, 4, 5} {:fuel TrimRight, 4, 5} {:fuel Trim, 4, 5}
        ParseMarkersReadsAMultiLineBlock()
    ensures var lines := ["<!-- REGEN: due", "  more", "-->", "body", "<!-- /REGEN -->"];
            ParseFrom(lines, 0) == [RegenMarker("due", "body")]
  {
    var lines := ["<!-- REGEN: due", "  more", "-->", "body", "<!-- /REGEN -->"];
    var t := lines[0];
    assert Trim(t) == t;
    assert !IsTemplateLine(t) by {
      if HasPrefix(t, TemplateTag) { assert t[..|TemplateTag|][5] == TemplateTag[5]; }
    }
    assert HasPrefix(t, RegenTag);
    assert t[|RegenTag|..] == " due";
    assert TrimRight(" due") == " due";
    assert !HasSuffix(" due", RegenArrow);
    assert !RegenOpenClosesOnItsLine(t);
    assert RegenCommand(t) == "due";
    // The arrow line is consumed; collection starts after it.
    assert Trim(lines[1]) == "more";
    assert !HasSuffix("more", RegenArrow);
    assert Trim(lines[2]) == "-->" && HasSuffix("-->", RegenArrow);
    assert SkipToArrow(lines, 1) == 3;
    assert Trim(lines[3]) == "body";
    assert Trim(lines[3]) != CloseLine by { assert lines[3][0] == 'b' && CloseLine[0] == '<'; }
    assert Trim(lines[4]) == CloseLine;
    assert SkipToClose(lines, 3) == 5;
    assert Content(lines, 3, 5) == "body" by {
      assert lines[3..4] == ["body"];
      assert Join(["body"], "\n") == "body";
    }
    assert ParseFrom(lines, 5) == [];
    assert ParseFrom(lines, 0) == [RegenMarker("due", "body")];
  }

  // The other half of `GroundedBySubstring` that no input satisfies: it requires a non-empty
  // instruction, and `<!-- TEMPLATE: -->` produces an empty one. `parse_markers` emits the
  // marker; the axiom said it could not exist.
  lemma {:fuel TrimLeft, 6, 7} {:fuel TrimRight, 6, 7} {:fuel Trim, 6, 7}
        AnEmptyTemplateInstructionIsStillAMarker()
    ensures var line := "<!-- TEMPLATE: -->";
            && ParseFrom([line], 0) == [TemplateMarker("")]
            && !GroundedBySubstring(line, TemplateMarker(""))
  {
    var line := "<!-- TEMPLATE: -->";
    assert Trim(line) == line;
    assert line[|TemplateTag|..] == " -->";
    assert HasSuffix(" -->", RegenArrow);
    assert IsTemplateLine(line);
    assert " -->"[..|" -->"| - |RegenArrow|] == " ";
    assert Trim(" ") == "";
    assert TemplateInstruction(line) == "";
    assert ParseFrom([line], 1) == [];
    assert ParseFrom([line], 0) == [TemplateMarker("")];
  }

  // The completeness predicate the file carried beside the soundness axiom: every REGEN block
  // in the text produces a marker. Nothing proved it, and it is false — which is the second
  // thing a definition with no consumer can be hiding.
  ghost predicate ParseMarkersComplete(text: string, markers: seq<Marker>) {
    forall cmd: string ::
      (|cmd| > 0 && (exists i :: SubstringAt(text, RegenOpen + cmd, i))) ==>
        (exists j :: 0 <= j < |markers| &&
          markers[j].RegenMarker? && markers[j].command == cmd)
  }

  // An unterminated REGEN block swallows every marker after it. The scan that looks for
  // "<!-- /REGEN -->" runs to the end of the file and takes the rest of the document as the
  // block's content, so a missing close tag does not report itself — it silently costs you
  // every marker below it.
  //
  // This is a defect in `parse_markers`, stated here rather than fixed here: changing the
  // parser is a change to three SDKs and their parity tests. Filed as #524, which will need
  // this lemma updated — it asserts the current output exactly, so it goes red on the fix.
  lemma {:fuel TrimLeft, 6, 7} {:fuel TrimRight, 6, 7} {:fuel Trim, 6, 7}
        ParseMarkersIsNotComplete()
    ensures var a := "<!-- REGEN: a -->";
            var b := "<!-- REGEN: b -->";
            && ParseFrom([a, b], 0) == [RegenMarker("a", b)]
            && (forall m :: m in ParseFrom([a, b], 0) ==> !m.RegenMarker? || m.command != "b")
  {
    var a := "<!-- REGEN: a -->";
    var b := "<!-- REGEN: b -->";
    assert Trim(a) == a && Trim(b) == b;
    assert !IsTemplateLine(a) by {
      if HasPrefix(a, TemplateTag) { assert a[..|TemplateTag|][5] == TemplateTag[5]; }
    }
    assert HasPrefix(a, RegenTag);
    assert a[|RegenTag|..] == " a -->";
    assert TrimRight(" a -->") == " a -->";
    assert RegenOpenClosesOnItsLine(a);
    assert " a -->"[..|" a -->"| - |RegenArrow|] == " a ";
    assert Trim(" a ") == "a";
    assert RegenCommand(a) == "a";
    assert Trim(b) != CloseLine by { assert b[5] == 'R' && CloseLine[5] == '/'; }
    assert SkipToClose([a, b], 1) == 2;
    assert Content([a, b], 1, 2) == b by { assert Join([b], "\n") == b; assert Trim(b) == b; }
    assert ParseFrom([a, b], 2) == [];
    assert ParseFrom([a, b], 0) == [RegenMarker("a", b)];
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
