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
// `markers.rs` transcribed: `RegenScan` reads blocks the way `scan_markers` does, and
// `UpdateRegen` rewrites every block the scan names, as `update_regen` does (#1097);
// `ParseFrom` walks lines the way `parse_markers` walks them. The one place the two scans
// differ is proved, not assumed: `TheModelsScanIsNotLineAnchored`.
//
// Modelling them found three claims **false as they were written** — two of them assumed by
// an axiom, which is what an axiom conceals, and one a predicate that stated a property and
// was then used by nothing at all. Each is now a proved refutation:
//   - `UpdateRegenSpec` claimed REGEN-block counts are preserved unconditionally. Counted as
//     open tags in the text, they are not: content can spell one. Counted as blocks the scan
//     reads, they are, and that is the clause now — `ContentThatSpellsATagIsNotABlock` is
//     the document that tells the two counts apart.
//   - `MarkerGrounded` (here `GroundedBySubstring`) claimed a marker's command appears in the
//     source as a raw substring after "<!-- REGEN: ". It does not — `parse_markers` trims, and
//     one extra space in the tag is enough. `TheSubstringFormOfGroundingIsFalse`.
//   - `ParseMarkersComplete` claimed every REGEN block yields a marker. An unterminated block
//     swallows every one below it: `ParseMarkersIsNotComplete`. Still true, and no longer
//     silent — #524 gave `scan_markers` a second channel, `ScanFrom` models it, and
//     `TheSwallowedBlockIsReported` proves the same input now names the block that took the
//     other one.
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
  // `update_regen` is defined over the scan (#1094): `scan_markers` records every well-formed
  // block's extent as it reads, and the writer rewrites the body of each block whose command
  // is *equal* to the one it was asked for. The model has the same shape (#1097). `RegenScan`
  // is the reader; `RegenSpan` and `UpdateRegen` are both selections over it. There is no
  // second search for them to disagree with, so *the two agree* is the definition, not a
  // lemma.

  const RegenOpen:  string := "<!-- REGEN: "
  const RegenArrow: string := "-->"
  const RegenClose: string := "<!-- /REGEN -->"

  lemma TagShapes()
    ensures forall i :: 0 <= i < |RegenClose| ==> RegenClose[i] != '\n'
    ensures forall i :: 0 <= i < |RegenOpen|  ==> RegenOpen[i]  != '\n'
  {}

  // The close tag begins again nowhere inside itself: '<' occurs in it once, at the front.
  //
  // This is what rules out an inline body whose tail, run together with the block's own close
  // tag, spells a close tag across the seam — the one way `ContainsNo(newContent, RegenClose)`
  // could be true of the content and false of the document it lands in. The block form never
  // needed it: its body is bracketed by newlines and neither tag holds one.
  lemma CloseTagHasOneAngle()
    ensures |RegenClose| == 15
    ensures RegenClose[0] == '<'
    ensures forall m :: 0 < m < |RegenClose| ==> RegenClose[m] != '<'
  {}

  // The four offsets, in the order the implementation computes them.
  datatype Span = Span(open: nat, arrow: nat, body: nat, close: nat)

  function End(sp: Span): nat { sp.close + |RegenClose| }

  function Shift(sp: Span, d: nat): Span {
    Span(sp.open + d, sp.arrow + d, sp.body + d, sp.close + d)
  }

  // The shape every span the scan reports has, relative to the string it was read from.
  predicate WellPlaced(s: string, sp: Span) {
    && sp.open + |RegenTag| <= sp.arrow
    && sp.body == sp.arrow + |RegenArrow|
    && sp.body <= sp.close
    && End(sp) <= |s|
  }

  // `WellPlaced(s, q)` of the span `Shift(q, base)`, stated without subtracting from a `nat`.
  predicate PlacedAt(s: string, sp: Span, base: nat) {
    && base <= sp.open
    && sp.open + |RegenTag| <= sp.arrow
    && sp.body == sp.arrow + |RegenArrow|
    && sp.body <= sp.close
    && End(sp) <= base + |s|
  }

  // The first block in `s`: the first open tag, the first arrow after it, and the first close
  // tag after that. This is the inline loop of `scan_markers` — `find` three times.
  //
  // The tag is `RegenTag`, without the space, because that is what `scan_markers` searches
  // for; the command is whatever lies between it and the arrow, trimmed.
  function NextBlock(s: string): Option<Span>
    ensures NextBlock(s).Some? ==>
      var sp := NextBlock(s).value;
      && WellPlaced(s, sp)
      && FindFrom(s, RegenTag, 0) == Some(sp.open)
      && FindFrom(s, RegenArrow, sp.open + |RegenTag|) == Some(sp.arrow)
      && FindFrom(s, RegenClose, sp.body) == Some(sp.close)
  {
    match FindFrom(s, RegenTag, 0)
      case None => None
      case Some(op) =>
        match FindFrom(s, RegenArrow, op + |RegenTag|)
          case None => None
          case Some(ai) =>
            match FindFrom(s, RegenClose, ai + |RegenArrow|)
              case None => None
              case Some(ci) => Some(Span(op, ai, ai + |RegenArrow|, ci))
  }

  function CommandAt(s: string, sp: Span): string
    requires WellPlaced(s, sp)
  { Trim(s[sp.open + |RegenTag|..sp.arrow]) }

  predicate NoNewline(s: string) { forall i :: 0 <= i < |s| ==> s[i] != '\n' }

  // The form a block was written in. RFC-0043 states the rule over the body's *text* rather
  // than over how the scan came to find the block, so that a cleared block-form section —
  // `-->\n<!-- /REGEN -->` — still reads as the block form it is.
  predicate InlineAt(text: string, sp: Span)
    requires sp.body <= sp.close <= |text|
  { NoNewline(text[sp.body..sp.close]) }

  // One well-formed block, as `scan_markers` records it — `RegenSpan` in `markers.rs`, which
  // carries the command, the extent, and the form. The name is taken here by the selection
  // below, which is older.
  datatype Block = Block(command: string, span: Span, inline: bool)

  // `scan_markers`' third output. Each block is found in the text after the previous one's
  // close tag, which is where the scan resumes — a body is never searched for an open tag.
  // `base` is how far into the original text `s` starts, so the spans are the original's.
  function ScanAt(s: string, base: nat): seq<Block>
    decreases |s|
  {
    match NextBlock(s)
      case None => []
      case Some(sp) =>
        [Block(CommandAt(s, sp), Shift(sp, base), InlineAt(s, sp))]
          + ScanAt(s[End(sp)..], base + End(sp))
  }

  // Every span the scan reports is where it says, in the string it read. A lemma rather than
  // a postcondition of `ScanAt`: as a postcondition it is a quantifier every proof that
  // mentions a scan pays for, and the inductions below time out paying it.
  lemma ScanIsPlaced(s: string, base: nat)
    ensures forall b :: b in ScanAt(s, base) ==> PlacedAt(s, b.span, base)
    decreases |s|
  {
    if NextBlock(s).Some? {
      var sp := NextBlock(s).value;
      ScanIsPlaced(s[End(sp)..], base + End(sp));
    }
  }

  // One step of the scan, as a fact a proof can call on. Left to the solver to unfold inside
  // a larger proof, this equation alone runs it out of time.
  lemma ScanAtSteps(s: string, base: nat, sp: Span)
    requires NextBlock(s) == Some(sp)
    ensures ScanAt(s, base)
         == [Block(CommandAt(s, sp), Shift(sp, base), InlineAt(s, sp))]
            + ScanAt(s[End(sp)..], base + End(sp))
  {}

  function RegenScan(text: string): seq<Block> { ScanAt(text, 0) }

  function Commands(blocks: seq<Block>): seq<string> {
    if |blocks| == 0 then [] else [blocks[0].command] + Commands(blocks[1..])
  }

  lemma CommandsOfCons(b: Block, rest: seq<Block>)
    ensures Commands([b] + rest) == [b.command] + Commands(rest)
  {
    assert ([b] + rest)[1..] == rest;
  }

  // The first block whose command is `command` — equal, not a prefix. Asked for `a`, a block
  // for `ab` is not an answer (`TheLocatorComparesTheWholeCommand`).
  function FirstWithCommand(blocks: seq<Block>, command: string): Option<Span>
    ensures FirstWithCommand(blocks, command).Some? ==>
      exists b :: b in blocks && b.command == command
                  && b.span == FirstWithCommand(blocks, command).value
    ensures FirstWithCommand(blocks, command).None? ==>
      forall b :: b in blocks ==> b.command != command
  {
    if |blocks| == 0 then None
    else if blocks[0].command == command then Some(blocks[0].span)
    else FirstWithCommand(blocks[1..], command)
  }

  lemma FirstWithCommandOfCons(b: Block, rest: seq<Block>, command: string)
    ensures FirstWithCommand([b] + rest, command)
         == if b.command == command then Some(b.span) else FirstWithCommand(rest, command)
  {
    assert ([b] + rest)[1..] == rest;
  }

  function RegenSpan(text: string, command: string): Option<Span>
    ensures RegenSpan(text, command).Some? ==> WellPlaced(text, RegenSpan(text, command).value)
  {
    ScanIsPlaced(text, 0);
    FirstWithCommand(RegenScan(text), command)
  }

  // A text has a block for `command` when the scan reads one.
  ghost predicate HasRegenFor(text: string, command: string) {
    exists b :: b in RegenScan(text) && b.command == command
  }

  // The selection misses no block the scan reads, and invents none.
  lemma RegenSpanFindsEveryBlock(text: string, command: string)
    ensures HasRegenFor(text, command) <==> RegenSpan(text, command).Some?
  {}

  lemma CommandsHoldsEveryCommand(blocks: seq<Block>, b: Block)
    requires b in blocks
    ensures b.command in Commands(blocks)
  {
    if blocks[0] != b {
      assert b in blocks[1..];
      CommandsHoldsEveryCommand(blocks[1..], b);
    }
  }

  // The body the implementation writes. Two special cases, and neither is tidiness.
  //
  // The empty case is collapsed so that clearing a block-form section does not leave a blank
  // line between the markers; a model that wrote "\n\n" would not be a model of it. The
  // inline case writes the content bare, which is what keeps a block in the middle of a
  // sentence in the middle of that sentence.
  //
  // `inline` alone does not decide the second one. A value holding a newline cannot be
  // written on one line, and writing it there anyway would leave a document whose body no
  // longer matches the form it declares — the *next* run would read that body as block form
  // and rewrite it. Reading both is what makes the idempotency clause of `UpdateRegenSpec`
  // true with no side condition on `newContent`.
  function RegenBody(newContent: string, inline: bool): string {
    if inline && NoNewline(newContent) then newContent
    else if newContent == "" then "\n"
    else "\n" + newContent + "\n"
  }

  // What a second run reads back out of a body the first run wrote is the form that body was
  // written in, so the second run writes the same body. This is the clause the two-condition
  // `RegenBody` is for: had it written a multi-line value into an inline block, the second
  // run would read block form where the first read inline, and lay down a different body.
  lemma RegenBodyKeepsItsForm(newContent: string, inline: bool)
    ensures RegenBody(newContent, NoNewline(RegenBody(newContent, inline)))
         == RegenBody(newContent, inline)
  {
    var body := RegenBody(newContent, inline);
    if !(inline && NoNewline(newContent)) {
      assert |body| > 0 && body[0] == '\n';
      assert !NoNewline(body);
    }
  }

  // What goes between a block's arrow and its close tag: the new body if the block is named
  // `command`, and what was there if not.
  function Mid(s: string, sp: Span, command: string, newContent: string): string
    requires WellPlaced(s, sp)
  {
    if CommandAt(s, sp) == command then RegenBody(newContent, InlineAt(s, sp))
    else s[sp.body..sp.close]
  }

  // `update_regen`: every block the scan reads with the command, rewritten — not the first.
  function Rewrite(s: string, command: string, newContent: string): string
    decreases |s|
  {
    match NextBlock(s)
      case None => s
      case Some(sp) =>
        s[..sp.body] + Mid(s, sp, command, newContent) + s[sp.close..End(sp)]
          + Rewrite(s[End(sp)..], command, newContent)
  }

  function UpdateRegen(text: string, command: string, newContent: string): string {
    Rewrite(text, command, newContent)
  }

  // Nothing in the freshly written body matches the close tag, provided the caller did not
  // write one into `newContent` — which is the precondition the spec below carries, and which
  // the axiom it replaced did not.
  //
  // Two shapes, because RFC-0043 gave the body two, and they fail differently. A block-form
  // body is bracketed by newlines and neither tag holds one, so nothing can straddle either
  // end and the precondition covers the inside. An inline body is the caller's string bare,
  // butted against the block's own close tag: the inside is still the precondition's
  // business, but the *seam* is new, and it is safe only because the close tag does not begin
  // again inside itself.
  lemma CloseTagNotInBody(result: string, nc: string, cs: nat, inline: bool)
    requires cs + |RegenBody(nc, inline)| <= |result|
    requires result[cs..cs + |RegenBody(nc, inline)|] == RegenBody(nc, inline)
    requires ContainsNo(nc, RegenClose)
    // The block's own close tag is what follows the body. In the block form this is not used;
    // in the inline form it is the other half of the seam.
    requires SubstringAt(result, RegenClose, cs + |RegenBody(nc, inline)|)
    ensures forall j :: cs <= j < cs + |RegenBody(nc, inline)| ==>
      !SubstringAt(result, RegenClose, j)
  {
    TagShapes();
    if inline && NoNewline(nc) {
      CloseTagHasOneAngle();
      assert RegenBody(nc, inline) == nc;
      forall m | 0 <= m < |nc| ensures result[cs + m] == nc[m] {
        assert result[cs..cs + |nc|][m] == nc[m];
      }
      forall j | cs <= j < cs + |nc|
        ensures !SubstringAt(result, RegenClose, j)
      {
        if SubstringAt(result, RegenClose, j) {
          if j + |RegenClose| <= cs + |nc| {
            // Wholly inside `nc`, which the precondition says holds no close tag.
            var k := j - cs;
            forall i | 0 <= i < |RegenClose| ensures nc[k + i] == RegenClose[i] {
              assert result[j..j + |RegenClose|][i] == RegenClose[i];
            }
            SliceIsLiteral(nc, k, k + |RegenClose|, RegenClose);
            assert SubstringAt(nc, RegenClose, k);
          } else {
            // Straddling the seam. The character at the seam is the close tag's first, by the
            // tag that follows the body; and it is the close tag's `m`th, by the match at `j`.
            // A tag with one '<' cannot have both.
            var m := cs + |nc| - j;
            assert 0 < m < |RegenClose|;
            assert result[cs + |nc|] == RegenClose[m] by {
              assert result[j..j + |RegenClose|][m] == RegenClose[m];
            }
            assert result[cs + |nc|] == RegenClose[0] by {
              assert result[cs + |nc|..cs + |nc| + |RegenClose|][0] == RegenClose[0];
            }
          }
        }
      }
      return;
    }
    var body := RegenBody(nc, inline);
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

  // ── The re-scan ───────────────────────────────────────────────────────────────
  //
  // Every clause of `UpdateRegenSpec` is a statement about the scan of the *result*, and the
  // result is the text with some bodies replaced. Replacing a body moves every offset after
  // it, so nothing about the text's scan carries over for free. What carries it is this: one
  // step of the rewrite, read back. The block the step wrote is the first block the scan of
  // the result finds, at the same open tag and arrow, with its close tag moved to the end of
  // the new body; and what follows that close tag is exactly what the step put there. Every
  // lemma below is an induction whose step is this one.

  lemma ReadBack(s: string, sp: Span, mid: string, tail: string)
    requires NextBlock(s) == Some(sp)
    requires forall j :: sp.body <= j < sp.body + |mid| ==>
      !SubstringAt(s[..sp.body] + mid + s[sp.close..End(sp)] + tail, RegenClose, j)
    ensures
      var r   := s[..sp.body] + mid + s[sp.close..End(sp)] + tail;
      var sp' := Span(sp.open, sp.arrow, sp.body, sp.body + |mid|);
      && NextBlock(r) == Some(sp')
      && CommandAt(r, sp') == CommandAt(s, sp)
      && InlineAt(r, sp') == NoNewline(mid)
      && r[..sp.body] == s[..sp.body]
      && r[sp.body..sp.body + |mid|] == mid
      && r[End(sp')..] == tail
  {
    var r   := s[..sp.body] + mid + s[sp.close..End(sp)] + tail;
    var sp' := Span(sp.open, sp.arrow, sp.body, sp.body + |mid|);
    assert r[..sp.body] == s[..sp.body];
    FindAgreesOnSharedPrefix(s, r, RegenTag, 0, sp.body);
    FindAgreesOnSharedPrefix(s, r, RegenArrow, sp.open + |RegenTag|, sp.body);
    assert r[sp'.close..End(sp')] == s[sp.close..End(sp)];
    assert SubstringAt(r, RegenClose, sp'.close);
    FindSkips(r, RegenClose, sp.body, sp'.close);
    SliceOfSharedPrefix(s, r, sp.open + |RegenTag|, sp.arrow, sp.body);
    assert r[sp.body..sp.body + |mid|] == mid;
    assert r[End(sp')..] == tail;
  }

  // The body a step writes holds no close tag, so `ReadBack` applies to every step `Rewrite`
  // takes. A rewritten body is `CloseTagNotInBody`'s business; an untouched one held none in
  // the text, because the scan took the *first* close tag after the arrow.
  lemma MidHoldsNoClose(s: string, sp: Span, command: string, nc: string, tail: string)
    requires NextBlock(s) == Some(sp)
    requires ContainsNo(nc, RegenClose)
    ensures
      var mid := Mid(s, sp, command, nc);
      var r   := s[..sp.body] + mid + s[sp.close..End(sp)] + tail;
      forall j :: sp.body <= j < sp.body + |mid| ==> !SubstringAt(r, RegenClose, j)
  {
    var mid := Mid(s, sp, command, nc);
    var r   := s[..sp.body] + mid + s[sp.close..End(sp)] + tail;
    if CommandAt(s, sp) == command {
      assert r[sp.body..sp.body + |mid|] == mid;
      assert SubstringAt(r, RegenClose, sp.body + |mid|) by {
        assert r[sp.body + |mid|..sp.body + |mid| + |RegenClose|] == s[sp.close..End(sp)];
      }
      CloseTagNotInBody(r, nc, sp.body, InlineAt(s, sp));
    } else {
      assert s[..sp.body] + s[sp.body..sp.close] + s[sp.close..End(sp)] == s[..End(sp)];
      assert r[..End(sp)] == s[..End(sp)];
      forall j | sp.body <= j < sp.body + |mid| ensures !SubstringAt(r, RegenClose, j) {
        SliceOfSharedPrefix(s, r, j, j + |RegenClose|, End(sp));
        assert !SubstringAt(s, RegenClose, j);
      }
    }
  }

  // One step of `Rewrite`, named, with `ReadBack` already applied to it.
  lemma Step(s: string, command: string, nc: string) returns (sp: Span, sp': Span)
    requires NextBlock(s).Some?
    requires ContainsNo(nc, RegenClose)
    ensures sp == NextBlock(s).value
    ensures
      var mid := Mid(s, sp, command, nc);
      var r   := Rewrite(s, command, nc);
      && sp' == Span(sp.open, sp.arrow, sp.body, sp.body + |mid|)
      && r == s[..sp.body] + mid + s[sp.close..End(sp)] + Rewrite(s[End(sp)..], command, nc)
      && NextBlock(r) == Some(sp')
      && CommandAt(r, sp') == CommandAt(s, sp)
      && InlineAt(r, sp') == NoNewline(mid)
      && r[..sp.body] == s[..sp.body]
      && r[sp.body..sp.body + |mid|] == mid
      && r[End(sp')..] == Rewrite(s[End(sp)..], command, nc)
  {
    sp := NextBlock(s).value;
    var mid := Mid(s, sp, command, nc);
    sp' := Span(sp.open, sp.arrow, sp.body, sp.body + |mid|);
    var tail := Rewrite(s[End(sp)..], command, nc);
    MidHoldsNoClose(s, sp, command, nc, tail);
    ReadBack(s, sp, mid, tail);
  }

  // Every block named `command` holds what a rewrite would write into it. Stated block by
  // block down the scan, so it says *every* and not *the first*: a model that rewrote only
  // the first match would leave a second one holding the old value, and fail this.
  predicate EveryWritten(s: string, command: string, nc: string)
    decreases |s|
  {
    match NextBlock(s)
      case None => true
      case Some(sp) =>
        && (CommandAt(s, sp) == command ==> s[sp.body..sp.close] == RegenBody(nc, InlineAt(s, sp)))
        && EveryWritten(s[End(sp)..], command, nc)
  }

  // A text whose every such block already holds the content is one `Rewrite` leaves alone.
  lemma RewriteFixesTheWritten(s: string, command: string, nc: string)
    requires EveryWritten(s, command, nc)
    ensures Rewrite(s, command, nc) == s
    decreases |s|
  {
    if NextBlock(s).Some? {
      var sp := NextBlock(s).value;
      RewriteFixesTheWritten(s[End(sp)..], command, nc);
      assert Mid(s, sp, command, nc) == s[sp.body..sp.close];
      assert s[..sp.body] + s[sp.body..sp.close] + s[sp.close..End(sp)] + s[End(sp)..] == s;
    }
  }

  // …and `Rewrite` produces one.
  lemma RewriteWritesEvery(s: string, command: string, nc: string)
    requires ContainsNo(nc, RegenClose)
    ensures EveryWritten(Rewrite(s, command, nc), command, nc)
    decreases |s|
  {
    if NextBlock(s).Some? {
      var sp, sp' := Step(s, command, nc);
      var r := Rewrite(s, command, nc);
      RewriteWritesEvery(s[End(sp)..], command, nc);
      if CommandAt(s, sp) == command {
        RegenBodyKeepsItsForm(nc, InlineAt(s, sp));
      }
    }
  }

  // The scan of the result reads the blocks the text had, with the same commands, in the
  // same order. The bases are free because the offsets are not the claim: every one after
  // the first rewritten body has moved.
  lemma RewriteKeepsTheScan(s: string, command: string, nc: string, b1: nat, b2: nat)
    requires ContainsNo(nc, RegenClose)
    ensures Commands(ScanAt(Rewrite(s, command, nc), b1)) == Commands(ScanAt(s, b2))
    decreases |s|
  {
    if NextBlock(s).Some? {
      var sp, sp' := Step(s, command, nc);
      var r := Rewrite(s, command, nc);
      var rest := s[End(sp)..];
      RewriteKeepsTheScan(rest, command, nc, b1 + End(sp'), b2 + End(sp));
      var rb := Block(CommandAt(r, sp'), Shift(sp', b1), InlineAt(r, sp'));
      var sb := Block(CommandAt(s, sp), Shift(sp, b2), InlineAt(s, sp));
      ScanAtSteps(r, b1, sp');
      ScanAtSteps(s, b2, sp);
      CommandsOfCons(rb, ScanAt(Rewrite(rest, command, nc), b1 + End(sp')));
      CommandsOfCons(sb, ScanAt(rest, b2 + End(sp)));
    } else {
      assert Rewrite(s, command, nc) == s;
    }
  }

  // The text with the body of every block named `command` taken out. Two texts that agree
  // here agree on every byte `update_regen` is not asked to write.
  function Hollow(s: string, command: string): string
    decreases |s|
  {
    match NextBlock(s)
      case None => s
      case Some(sp) =>
        s[..sp.body] + (if CommandAt(s, sp) == command then "" else s[sp.body..sp.close])
          + s[sp.close..End(sp)] + Hollow(s[End(sp)..], command)
  }

  lemma RewriteTouchesOnlyTheBodies(s: string, command: string, nc: string)
    requires ContainsNo(nc, RegenClose)
    ensures Hollow(Rewrite(s, command, nc), command) == Hollow(s, command)
    decreases |s|
  {
    if NextBlock(s).Some? {
      var sp, sp' := Step(s, command, nc);
      var r := Rewrite(s, command, nc);
      RewriteTouchesOnlyTheBodies(s[End(sp)..], command, nc);
      assert r[sp'.close..End(sp')] == s[sp.close..End(sp)];
    }
  }

  // The first block named `command`, before and after. Everything up to its body is
  // untouched, its body is the new one, its close tag follows directly, and the scan of the
  // result selects it at the same open tag and arrow. `q` is the block's span relative to `s`
  // — returned rather than computed in the postcondition, where subtracting `base` from a
  // `nat` would need this lemma's conclusion to be well-formed.
  lemma RewriteAtTheFirstMatch(s: string, command: string, nc: string, base: nat)
    returns (q: Span)
    requires ContainsNo(nc, RegenClose)
    requires FirstWithCommand(ScanAt(s, base), command).Some?
    ensures FirstWithCommand(ScanAt(s, base), command) == Some(Shift(q, base))
    ensures WellPlaced(s, q)
    ensures
      var r    := Rewrite(s, command, nc);
      var body := RegenBody(nc, InlineAt(s, q));
      && q.body + |body| + |RegenClose| <= |r|
      && r[..q.body] == s[..q.body]
      && r[q.body..q.body + |body|] == body
      && SubstringAt(r, RegenClose, q.body + |body|)
      && FirstWithCommand(ScanAt(r, base), command)
         == Some(Shift(Span(q.open, q.arrow, q.body, q.body + |body|), base))
    decreases |s|, 1
  {
    ScanAtSteps(s, base, NextBlock(s).value);
    var sp := NextBlock(s).value;
    FirstWithCommandOfCons(Block(CommandAt(s, sp), Shift(sp, base), InlineAt(s, sp)),
                           ScanAt(s[End(sp)..], base + End(sp)), command);
    if CommandAt(s, sp) == command {
      q := FirstMatchIsHere(s, command, nc, base);
    } else {
      q := FirstMatchIsLater(s, command, nc, base);
    }
  }

  // The first block is the one: `Step` has already said everything.
  lemma FirstMatchIsHere(s: string, command: string, nc: string, base: nat) returns (q: Span)
    requires NextBlock(s).Some? && CommandAt(s, NextBlock(s).value) == command
    requires ContainsNo(nc, RegenClose)
    requires FirstWithCommand(ScanAt(s, base), command).Some?
    ensures FirstWithCommand(ScanAt(s, base), command) == Some(Shift(q, base))
    ensures WellPlaced(s, q)
    ensures
      var r    := Rewrite(s, command, nc);
      var body := RegenBody(nc, InlineAt(s, q));
      && q.body + |body| + |RegenClose| <= |r|
      && r[..q.body] == s[..q.body]
      && r[q.body..q.body + |body|] == body
      && SubstringAt(r, RegenClose, q.body + |body|)
      && FirstWithCommand(ScanAt(r, base), command)
         == Some(Shift(Span(q.open, q.arrow, q.body, q.body + |body|), base))
  {
    var sp, sp' := Step(s, command, nc);
    var r := Rewrite(s, command, nc);
    q := sp;
    ScanAtSteps(s, base, sp);
    ScanAtSteps(r, base, sp');
    FirstWithCommandOfCons(Block(CommandAt(s, sp), Shift(sp, base), InlineAt(s, sp)),
                           ScanAt(s[End(sp)..], base + End(sp)), command);
    FirstWithCommandOfCons(Block(CommandAt(r, sp'), Shift(sp', base), InlineAt(r, sp')),
                           ScanAt(r[End(sp')..], base + End(sp')), command);
    assert r[sp'.close..End(sp')] == s[sp.close..End(sp)];
  }

  // The first block is not the one, so the step left it as it was and the answer is in the
  // rest — at the same place, moved by the length of the block in front of it.
  lemma FirstMatchIsLater(s: string, command: string, nc: string, base: nat) returns (q: Span)
    requires NextBlock(s).Some? && CommandAt(s, NextBlock(s).value) != command
    requires ContainsNo(nc, RegenClose)
    requires FirstWithCommand(ScanAt(s, base), command).Some?
    ensures FirstWithCommand(ScanAt(s, base), command) == Some(Shift(q, base))
    ensures WellPlaced(s, q)
    ensures
      var r    := Rewrite(s, command, nc);
      var body := RegenBody(nc, InlineAt(s, q));
      && q.body + |body| + |RegenClose| <= |r|
      && r[..q.body] == s[..q.body]
      && r[q.body..q.body + |body|] == body
      && SubstringAt(r, RegenClose, q.body + |body|)
      && FirstWithCommand(ScanAt(r, base), command)
         == Some(Shift(Span(q.open, q.arrow, q.body, q.body + |body|), base))
    decreases |s|, 0
  {
    hide Rewrite, ScanAt;
    var sp := NextBlock(s).value;
    var e := End(sp);
    var rest := s[e..];
    var tail := Rewrite(rest, command, nc);
    var r := Rewrite(s, command, nc);
    RewriteSkips(s, command, nc);
    var sb := Block(CommandAt(s, sp), Shift(sp, base), InlineAt(s, sp));
    var rb := Block(CommandAt(r, sp), Shift(sp, base), InlineAt(r, sp));
    ScanAtSteps(s, base, sp);
    ScanAtSteps(r, base, sp);
    FirstWithCommandOfCons(sb, ScanAt(rest, base + e), command);
    FirstWithCommandOfCons(rb, ScanAt(tail, base + e), command);
    assert FirstWithCommand(ScanAt(rest, base + e), command)
        == FirstWithCommand(ScanAt(s, base), command);
    assert |rest| < |s|;
    assert FirstWithCommand(ScanAt(rest, base + e), command).Some?;
    var q' := RewriteAtTheFirstMatch(rest, command, nc, base + e);
    q := Shift(q', e);
    assert Shift(q, base) == Shift(q', base + e);
    SuffixSlice(s, e, q'.body, q'.close);
    assert InlineAt(s, q) == InlineAt(rest, q');
    var body := RegenBody(nc, InlineAt(s, q));
    assert Shift(Span(q.open, q.arrow, q.body, q.body + |body|), base)
        == Shift(Span(q'.open, q'.arrow, q'.body, q'.body + |body|), base + e);
    PrefixedSlices(s[..e], tail, q'.body, q'.body + |body|);
    PrefixedSlices(s[..e], tail, q'.body + |body|, q'.body + |body| + |RegenClose|);
    PrefixedSlices(s[..e], rest, q'.body, q'.body);
    assert s[..e] + rest == s;
  }

  // A step over a block not named `command` copies it whole.
  lemma RewriteSkips(s: string, command: string, nc: string)
    requires ContainsNo(nc, RegenClose)
    requires NextBlock(s).Some? && CommandAt(s, NextBlock(s).value) != command
    ensures
      var sp := NextBlock(s).value;
      var r  := Rewrite(s, command, nc);
      && r == s[..End(sp)] + Rewrite(s[End(sp)..], command, nc)
      && NextBlock(r) == Some(sp)
      && CommandAt(r, sp) == CommandAt(s, sp)
      && r[End(sp)..] == Rewrite(s[End(sp)..], command, nc)
  {
    var sp, sp' := Step(s, command, nc);
    assert s[..sp.body] + s[sp.body..sp.close] + s[sp.close..End(sp)] == s[..End(sp)];
  }

  lemma SuffixSlice(s: string, e: nat, a: nat, b: nat)
    requires e <= |s| && a <= b <= |s| - e
    ensures s[e..][a..b] == s[e + a..e + b]
  {}

  lemma PrefixedSlices(p: string, t: string, a: nat, b: nat)
    requires a <= b <= |t|
    ensures (p + t)[|p| + a..|p| + b] == t[a..b]
    ensures (p + t)[..|p| + a] == p + t[..a]
  {}

  // **Claim (1).** Content preservation and idempotency, over the scan.
  //
  // Five clauses, and the third is the one the axiom this replaced had and lost. "No REGEN
  // blocks created or destroyed" was stated over a count of substrings, and it was false:
  // content spelling an open tag adds one. Over the scan it is true — a body is never searched
  // for an open tag, so a tag written into one is content and not a block —
  // `ContentThatSpellsATagIsNotABlock` is that document.
  //
  // `ContainsNo(newContent, RegenClose)` is the price of the rest: a caller who writes a close
  // tag into the new content terminates the section early, and every clause below is false of
  // that call.
  lemma UpdateRegenSpec(text: string, command: string, newContent: string)
    requires HasRegenFor(text, command)
    requires ContainsNo(newContent, RegenClose)
    ensures RegenSpan(text, command).Some?
    ensures
      var sp     := RegenSpan(text, command).value;
      var result := UpdateRegen(text, command, newContent);
      var body   := RegenBody(newContent, InlineAt(text, sp));
      && sp.body + |body| <= |result|
      // (1) Frame: nothing before the first such block's body moves, and nothing anywhere
      //     that is not the body of a block named `command` changes.
      && result[..sp.body] == text[..sp.body]
      && Hollow(result, command) == Hollow(text, command)
      // (2) The first such block holds the new body and is still selected, at the same open
      //     tag and arrow, with its close tag right after the body.
      && result[sp.body..sp.body + |body|] == body
      && RegenSpan(result, command) == Some(Span(sp.open, sp.arrow, sp.body, sp.body + |body|))
      && HasRegenFor(result, command)
      // (3) The scan of the result reads the blocks the text had, in the same order.
      && Commands(RegenScan(result)) == Commands(RegenScan(text))
      // (4) Every block named `command` holds the new content — every, not the first.
      && EveryWritten(result, command, newContent)
      // (5) Idempotency: a second application with the same content changes nothing.
      && UpdateRegen(result, command, newContent) == result
  {
    RegenSpanFindsEveryBlock(text, command);
    var sp := RegenSpan(text, command).value;
    var q := RewriteAtTheFirstMatch(text, command, newContent, 0);
    assert Shift(q, 0) == q;
    var body := RegenBody(newContent, InlineAt(text, sp));
    assert Shift(Span(sp.open, sp.arrow, sp.body, sp.body + |body|), 0)
        == Span(sp.open, sp.arrow, sp.body, sp.body + |body|);
    var result := UpdateRegen(text, command, newContent);
    RegenSpanFindsEveryBlock(result, command);
    RewriteTouchesOnlyTheBodies(text, command, newContent);
    RewriteKeepsTheScan(text, command, newContent, 0, 0);
    RewriteWritesEvery(text, command, newContent);
    RewriteFixesTheWritten(result, command, newContent);
  }

  // ── Witnesses ─────────────────────────────────────────────────────────────────
  //
  // The lemmas above are universal, and a universal statement about the empty set reads
  // exactly like one about something. Each witness below is one document with its offsets
  // worked out. Every block in them starts at offset 0 on purpose: it keeps each `FindFrom`
  // to a handful of unrollings rather than a scan across a sentence.

  // `update_regen` collapses the empty body: clearing a block-form section leaves the two
  // markers on consecutive lines rather than with a blank line between them. The spec is
  // written in terms of `RegenBody`, so a model that wrote "\n\n" would satisfy every clause
  // of it. This is what fails if `RegenBody` stops special-casing the empty string, and it is
  // the only thing that does: it names the close tag's offset, not the body's.
  lemma ClearingASectionLeavesNoBlankLine(text: string, command: string)
    requires RegenSpan(text, command).Some?
    requires !InlineAt(text, RegenSpan(text, command).value)
    ensures var sp := RegenSpan(text, command).value;
            var result := UpdateRegen(text, command, "");
            && sp.body + 1 <= |result|
            && result[..sp.body + 1] == text[..sp.body] + "\n"
            && SubstringAt(result, RegenClose, sp.body + 1)
  {
    var sp := RegenSpan(text, command).value;
    assert ContainsNo("", RegenClose);
    var q := RewriteAtTheFirstMatch(text, command, "", 0);
    assert Shift(q, 0) == q;
    assert RegenBody("", false) == "\n";
    var result := UpdateRegen(text, command, "");
    assert result[..sp.body + 1] == result[..sp.body] + result[sp.body..sp.body + 1];
  }

  // The same question asked of the other form, and it has the other answer: clearing an
  // inline block writes nothing between its markers at all — not even the newline the block
  // form collapses to. An inline block sits inside a sentence, and a newline there would
  // break the sentence, which is the defect RFC-0043 is about in its smallest form.
  //
  // The precondition above and the precondition here partition the blocks a text has: every
  // block is one form or the other, and neither lemma is about the empty set of them.
  // `AnInlineBlockIsNotAnEmptyCase` supplies the witness this one needs.
  lemma ClearingAnInlineSectionWritesNothing(text: string, command: string)
    requires RegenSpan(text, command).Some?
    requires InlineAt(text, RegenSpan(text, command).value)
    ensures var sp := RegenSpan(text, command).value;
            var result := UpdateRegen(text, command, "");
            && sp.body <= |result|
            && result[..sp.body] == text[..sp.body]
            && SubstringAt(result, RegenClose, sp.body)
  {
    assert ContainsNo("", RegenClose);
    var q := RewriteAtTheFirstMatch(text, command, "", 0);
    assert Shift(q, 0) == q;
    assert RegenBody("", true) == "";
  }

  // The scan reads nothing from an empty rest. Every witness ends its document on a close
  // tag, and this is the step that says the scan stops there.
  lemma NothingAfterTheEnd(base: nat)
    ensures NextBlock("") == None
    ensures ScanAt("", base) == []
    ensures forall c, nc :: Rewrite("", c, nc) == ""
  {
    assert FindFrom("", RegenTag, 0) == None;
  }

  // The characters and slices of the four witness documents below, each proved alone. Proved
  // in place, next to the scan's definitions, the solver sometimes fails to index a string
  // literal it indexes instantly on its own.
  lemma InlineDocument()
    ensures var text := "<!-- REGEN: n -->44<!-- /REGEN -->";
            && text[11] == ' ' && text[12] == 'n' && text[13] == ' '
            && text[17] == '4' && text[18] == '4'
            && text[0..11] == "<!-- REGEN:"
            && text[0..17] == "<!-- REGEN: n -->"
            && text[11..14] == " n "
            && text[14..17] == "-->"
            && text[17..19] == "44"
            && text[19..34] == "<!-- /REGEN -->"
  {
    var text := "<!-- REGEN: n -->44<!-- /REGEN -->";
    SliceIsLiteral(text, 0, 11, "<!-- REGEN:");
    SliceIsLiteral(text, 0, 17, "<!-- REGEN: n -->");
    SliceIsLiteral(text, 11, 14, " n ");
    SliceIsLiteral(text, 14, 17, "-->");
    SliceIsLiteral(text, 17, 19, "44");
    SliceIsLiteral(text, 19, 34, "<!-- /REGEN -->");
  }

  lemma PrefixDocument()
    ensures var text := "<!-- REGEN: ab -->1<!-- /REGEN -->";
            && text[11] == ' ' && text[12] == 'a' && text[13] == 'b' && text[14] == ' '
            && text[18] == '1'
            && text[0..11] == "<!-- REGEN:"
            && text[11..15] == " ab "
            && text[15..18] == "-->"
            && text[19..34] == "<!-- /REGEN -->"
  {
    var text := "<!-- REGEN: ab -->1<!-- /REGEN -->";
    SliceIsLiteral(text, 0, 11, "<!-- REGEN:");
    SliceIsLiteral(text, 11, 15, " ab ");
    SliceIsLiteral(text, 15, 18, "-->");
    SliceIsLiteral(text, 19, 34, "<!-- /REGEN -->");
  }

  lemma BlockDocument()
    ensures var text := "<!-- REGEN: a -->\n\n<!-- /REGEN -->";
            && text[11] == ' ' && text[12] == 'a' && text[13] == ' '
            && text[17] == '\n' && text[18] == '\n'
            && text[0..11] == "<!-- REGEN:"
            && text[11..14] == " a "
            && text[14..17] == "-->"
            && text[19..34] == "<!-- /REGEN -->"
  {
    var text := "<!-- REGEN: a -->\n\n<!-- /REGEN -->";
    SliceIsLiteral(text, 0, 11, "<!-- REGEN:");
    SliceIsLiteral(text, 11, 14, " a ");
    SliceIsLiteral(text, 14, 17, "-->");
    SliceIsLiteral(text, 19, 34, "<!-- /REGEN -->");
  }

  lemma UnanchoredDocument()
    ensures var text := "<!-- REGEN: a -->\nx<!-- /REGEN -->";
            && text[11] == ' ' && text[12] == 'a' && text[13] == ' '
            && text[17] == '\n' && text[18] == 'x'
            && text[0..11] == "<!-- REGEN:"
            && text[11..14] == " a "
            && text[14..17] == "-->"
            && text[19..34] == "<!-- /REGEN -->"
  {
    var text := "<!-- REGEN: a -->\nx<!-- /REGEN -->";
    SliceIsLiteral(text, 0, 11, "<!-- REGEN:");
    SliceIsLiteral(text, 11, 14, " a ");
    SliceIsLiteral(text, 14, 17, "-->");
    SliceIsLiteral(text, 19, 34, "<!-- /REGEN -->");
  }

  // `ClearingAnInlineSectionWritesNothing` is about something: one inline block, read and
  // rewritten.
  lemma AnInlineBlockIsNotAnEmptyCase()
    ensures var text := "<!-- REGEN: n -->44<!-- /REGEN -->";
            && RegenScan(text) == [Block("n", Span(0, 14, 17, 19), true)]
            && RegenSpan(text, "n") == Some(Span(0, 14, 17, 19))
            && InlineAt(text, Span(0, 14, 17, 19))
            && UpdateRegen(text, "n", "43") == "<!-- REGEN: n -->43<!-- /REGEN -->"
  {
    var text := "<!-- REGEN: n -->44<!-- /REGEN -->";
    InlineDocument();
    var sp := Span(0, 14, 17, 19);
    assert FindFrom(text, RegenTag, 0) == Some(0);

    // The arrow: 11, 12 and 13 are the space, the `n`, and the space.
    assert !SubstringAt(text, RegenArrow, 11) by { assert text[11..14][0] == ' '; }
    assert !SubstringAt(text, RegenArrow, 12) by { assert text[12..15][0] == 'n'; }
    assert !SubstringAt(text, RegenArrow, 13) by { assert text[13..16][0] == ' '; }
    assert FindFrom(text, RegenArrow, 11) == Some(14);

    // The close tag: not at 17 or 18 (the digits), so at 19.
    assert !SubstringAt(text, RegenClose, 17) by { assert text[17..32][0] == '4'; }
    assert !SubstringAt(text, RegenClose, 18) by { assert text[18..33][0] == '4'; }
    assert FindFrom(text, RegenClose, 17) == Some(19);
    assert NextBlock(text) == Some(sp);

    assert Trim(" n ") == "n";
    assert CommandAt(text, sp) == "n";
    assert text[17..19] == "44";
    assert InlineAt(text, sp);
    NothingAfterTheEnd(34);
    assert text[34..] == "";
    assert RegenScan(text) == [Block("n", sp, true)];

    assert RegenBody("43", true) == "43";
    assert Mid(text, sp, "n", "43") == "43";
    assert UpdateRegen(text, "n", "43") == text[..17] + "43" + text[19..34] + "";
  }

  // Asked for `a`, a block for `ab` is not an answer: the command is compared whole. This is
  // #1058's clobber in its smallest form, and the model used to commit it — `RegenSpan` was a
  // prefix search of its own, and on this document it answered with the `ab` block. Now it
  // answers with nothing, as `update_regen` does, and rewriting `a` leaves the document alone.
  // `a_query_that_prefixes_another_does_not_clobber_it` in `tests/count.rs` is the same
  // document, run against the code.
  lemma TheLocatorComparesTheWholeCommand()
    ensures var text := "<!-- REGEN: ab -->1<!-- /REGEN -->";
            && RegenSpan(text, "a") == None
            && RegenSpan(text, "ab") == Some(Span(0, 15, 18, 19))
            && UpdateRegen(text, "a", "2") == text
  {
    var text := "<!-- REGEN: ab -->1<!-- /REGEN -->";
    PrefixDocument();
    var sp := Span(0, 15, 18, 19);
    assert FindFrom(text, RegenTag, 0) == Some(0);

    // The arrow: 11 to 14 are the space, `a`, `b`, and the space.
    assert !SubstringAt(text, RegenArrow, 11) by { assert text[11..14][0] == ' '; }
    assert !SubstringAt(text, RegenArrow, 12) by { assert text[12..15][0] == 'a'; }
    assert !SubstringAt(text, RegenArrow, 13) by { assert text[13..16][0] == 'b'; }
    assert !SubstringAt(text, RegenArrow, 14) by { assert text[14..17][0] == ' '; }
    assert FindFrom(text, RegenArrow, 11) == Some(15);

    // The close tag: offset 18 is the body.
    assert !SubstringAt(text, RegenClose, 18) by { assert text[18..33][0] == '1'; }
    assert FindFrom(text, RegenClose, 18) == Some(19);
    assert NextBlock(text) == Some(sp);

    assert Trim(" ab ") == "ab";
    assert CommandAt(text, sp) == "ab";
    NothingAfterTheEnd(34);
    assert text[34..] == "";
    assert RegenScan(text) == [Block("ab", sp, InlineAt(text, sp))];

    assert Mid(text, sp, "a", "2") == text[18..19];
    assert UpdateRegen(text, "a", "2") == text[..18] + text[18..19] + text[19..34] + "";
  }

  // Where the one block in `ContentThatSpellsATagIsNotABlock`'s document sits, and which form
  // it is in. Separate from the lemma it serves: pinning a span is a dozen facts about one
  // string literal, and proved in place they crowd out the facts that lemma is about.
  lemma TheOneBlockIsBlockForm()
    ensures var text := "<!-- REGEN: a -->\n\n<!-- /REGEN -->";
            RegenScan(text) == [Block("a", Span(0, 14, 17, 19), false)]
  {
    var text := "<!-- REGEN: a -->\n\n<!-- /REGEN -->";
    BlockDocument();
    var sp := Span(0, 14, 17, 19);
    assert FindFrom(text, RegenTag, 0) == Some(0);

    // The arrow: 11, 12 and 13 are the space, the `a`, and the space.
    assert !SubstringAt(text, RegenArrow, 11) by { assert text[11..14][0] == ' '; }
    assert !SubstringAt(text, RegenArrow, 12) by { assert text[12..15][0] == 'a'; }
    assert !SubstringAt(text, RegenArrow, 13) by { assert text[13..16][0] == ' '; }
    assert FindFrom(text, RegenArrow, 11) == Some(14);

    // The close tag: offsets 17 and 18 are the body's two newlines, so the first match is 19.
    assert !SubstringAt(text, RegenClose, 17) by { assert text[17..32][0] == '\n'; }
    assert !SubstringAt(text, RegenClose, 18) by { assert text[18..33][0] == '\n'; }
    assert FindFrom(text, RegenClose, 17) == Some(19);
    assert NextBlock(text) == Some(sp);

    assert Trim(" a ") == "a" by {
      assert TrimRight(" a ") == " a";
      assert TrimLeft(" a") == "a";
    }
    assert CommandAt(text, sp) == "a";
    assert !InlineAt(text, sp) by { assert text[17..19][0] == '\n'; }
    NothingAfterTheEnd(34);
    assert text[34..] == "";
  }

  // `UpdateRegenSpec` is about something. A lemma whose preconditions no input satisfies
  // proves its postcondition of nothing at all, and reads exactly like one that does.
  lemma UpdateRegenSpecIsNotVacuous()
    ensures var text := "<!-- REGEN: a -->\n\n<!-- /REGEN -->";
            HasRegenFor(text, "a") && ContainsNo("new", RegenClose)
  {
    TheOneBlockIsBlockForm();
    var text := "<!-- REGEN: a -->\n\n<!-- /REGEN -->";
    assert RegenScan(text)[0] in RegenScan(text);
    forall i | 0 <= i <= |"new"| ensures !SubstringAt("new", RegenClose, i) {}
  }

  // Clause (3) of the axiom — "No REGEN blocks created or destroyed" — was stated over a
  // count of open tags anywhere in the text, and it was false: a caller passing an open tag
  // as `new_content` raised that count by one. It is clause (3) of `UpdateRegenSpec` again
  // now, stated over the scan, and true — because the scan never searches a body.
  //
  // This is the document that tells the two apart. The open tag for `b` is in the result,
  // where the old count saw it; the scan of the result reads one block, `a`, as it did before.
  // The code agrees, and says more: `scan_markers` reads this document line by line, sees an
  // open tag inside `a`'s body, and reports `a` as `ClosedOnAnothersTag` —
  // `ABlockThatClosesOnAnothersTagIsReported` is that report, in the line model.
  lemma ContentThatSpellsATagIsNotABlock()
    ensures var text   := "<!-- REGEN: a -->\n\n<!-- /REGEN -->";
            var nc     := "<!-- REGEN: b -->";
            var result := UpdateRegen(text, "a", nc);
            && ContainsNo(nc, RegenClose)
            && |result| >= 18 + |RegenOpen + "b"|
            && SubstringAt(result, RegenOpen + "b", 18)
            && Commands(RegenScan(result)) == ["a"]
            && !HasRegenFor(result, "b")
  {
    var text := "<!-- REGEN: a -->\n\n<!-- /REGEN -->";
    var nc   := "<!-- REGEN: b -->";
    TheOneBlockIsBlockForm();
    var sp := Span(0, 14, 17, 19);
    assert RegenScan(text)[0] in RegenScan(text);
    // `nc` holds no '/', and the close tag is '/' at offset 5.
    assert RegenClose[5] == '/';
    forall i | 0 <= i <= |nc| ensures !SubstringAt(nc, RegenClose, i) {
      if SubstringAt(nc, RegenClose, i) { assert nc[i..i + |RegenClose|][5] == nc[i + 5]; }
    }
    UpdateRegenSpec(text, "a", nc);
    assert RegenSpan(text, "a") == Some(sp);
    var result := UpdateRegen(text, "a", nc);
    var body := RegenBody(nc, false);
    assert body == "\n" + nc + "\n";
    assert result[17..17 + |body|] == body;
    assert result[18..18 + |nc|] == nc by {
      assert forall i {:trigger body[1 + i]} :: 0 <= i < |nc| ==> result[18 + i] == nc[i];
      SliceIsLiteral(result, 18, 18 + |nc|, nc);
    }
    assert SubstringAt(result, RegenOpen + "b", 18) by {
      SliceIsLiteral(nc, 0, 13, "<!-- REGEN: b");
      SliceIsLiteral(result, 18, 31, "<!-- REGEN: b");
    }
    assert Commands(RegenScan(text)) == ["a"];
    if HasRegenFor(result, "b") {
      var b :| b in RegenScan(result) && b.command == "b";
      CommandsHoldsEveryCommand(RegenScan(result), b);
    }
  }

  // ── what this model does not yet say ──────────────────────────────────────────
  //
  // `NextBlock` is `scan_markers`' inline loop without the line: for a block on one line it
  // is what the code does. A block *across* lines the code reads a line at a time, and holds
  // it to a discipline the model does not have. The open tag has to start its line, a
  // multi-line open tag ends on a line that ends in `-->`, and the close tag has to stand
  // alone on its line. The model reads a block wherever the three tags occur in that order.
  //
  // So where the code rejects a candidate, the model can read a block the code does not.
  // The smallest case is below: a close tag with text before it on its line. The code reads
  // that block as `CloseTagMissing` and records no extent, so `update_regen` leaves the
  // document alone; the model rewrites it. The rejection can also change *which* block is
  // read. An open tag mid-sentence with no close tag beside it is prose to the code, and
  // the next block down is the one it reads. The model reads a block from the prose tag to
  // that next block's close tag.
  //
  // `a_close_tag_that_does_not_stand_alone_is_not_a_block_form_close` in `markers.rs` is
  // the document below, run against the code, so a change to either side reddens one of
  // them. Closing the gap means modelling `lines_with_offsets` and the block-form branch.
  // `ReadBack` would need proving again for the new `NextBlock`. The inductions built on it
  // would not change.
  lemma TheModelsScanIsNotLineAnchored()
    ensures var text := "<!-- REGEN: a -->\nx<!-- /REGEN -->";
            RegenSpan(text, "a") == Some(Span(0, 14, 17, 19))
  {
    var text := "<!-- REGEN: a -->\nx<!-- /REGEN -->";
    UnanchoredDocument();
    var sp := Span(0, 14, 17, 19);
    assert FindFrom(text, RegenTag, 0) == Some(0);
    assert !SubstringAt(text, RegenArrow, 11) by { assert text[11..14][0] == ' '; }
    assert !SubstringAt(text, RegenArrow, 12) by { assert text[12..15][0] == 'a'; }
    assert !SubstringAt(text, RegenArrow, 13) by { assert text[13..16][0] == ' '; }
    assert FindFrom(text, RegenArrow, 11) == Some(14);
    // The close tag: 17 is the newline, 18 is the `x` the code objects to.
    assert !SubstringAt(text, RegenClose, 17) by { assert text[17..32][0] == '\n'; }
    assert !SubstringAt(text, RegenClose, 18) by { assert text[18..33][0] == 'x'; }
    assert FindFrom(text, RegenClose, 17) == Some(19);
    assert NextBlock(text) == Some(sp);
    assert Trim(" a ") == "a" by {
      assert TrimRight(" a ") == " a";
      assert TrimLeft(" a") == "a";
    }
    assert CommandAt(text, sp) == "a";
    assert RegenScan(text)[0] == Block("a", sp, InlineAt(text, sp));
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
  // ── What the scan could not read the way it was meant (#524) ─────────────────
  //
  // `ParseMarkersIsNotComplete` below proves that a block running past its own end silently
  // takes the markers under it. `scan_markers` reports that rather than leaving it to be
  // inferred from markers that are absent, and this is the model of the second channel.
  //
  // Three faults, because there are three ways a block's extent goes wrong, and only one of
  // them needs the damaged block to be last in the document — which is why it is the one that
  // is easy to miss and the other is the one a real file has.

  datatype Fault = OpenArrowMissing | CloseTagMissing | ClosedOnAnothersTag

  datatype MalformedBlock = MalformedBlock(command: string, line: nat, fault: Fault)

  // A body holding an open tag of its own means a close tag is missing above it: the tag this
  // block closed on is the inner block's.
  ghost predicate BodyOpensARegen(lines: seq<string>, j: nat, k: nat)
    requires j <= |lines| && k <= |lines|
  {
    exists m :: j <= m < k && HasPrefix(Trim(lines[m]), RegenTag)
  }

  predicate BodyOpensARegenD(lines: seq<string>, j: nat, k: nat)
    requires j <= k <= |lines|
    decreases k - j
  {
    if j == k then false
    else HasPrefix(Trim(lines[j]), RegenTag) || BodyOpensARegenD(lines, j + 1, k)
  }

  // The order is not arbitrary: an open tag that never closed has no body to inspect, and a
  // block that ran off the end has no close tag to have taken from anyone. Each test is only
  // meaningful once the ones above it have failed.
  //
  // `k - 1` in the third test excludes the close tag the block landed on. Widening it to `k`
  // is an **equivalent mutation** — it survives every witness here, and it should: in every
  // state that reaches this branch `lines[k - 1]` is the close tag, which does not open a
  // REGEN block. Recorded rather than papered over with a witness that would only be
  // asserting the two spellings agree.
  function BlockFault(lines: seq<string>, i: nat, j: nat, k: nat): Option<Fault>
    requires i <= j <= k <= |lines|
  {
    if j == |lines| && !(j > i && HasSuffix(Trim(lines[j - 1]), RegenArrow))
      then Some(OpenArrowMissing)
    else if k == |lines| && !(k > j && Trim(lines[k - 1]) == CloseLine)
      then Some(CloseTagMissing)
    else if BodyOpensARegenD(lines, j, if k > j then k - 1 else k)
      then Some(ClosedOnAnothersTag)
    else None
  }

  datatype Scan = Scan(markers: seq<Marker>, malformed: seq<MalformedBlock>)

  // The scan, with both of its outputs. Each carries its own grounding postcondition: a
  // marker comes from a line that opens it, and a report points at a line that opens a REGEN
  // block. Neither can be invented, which is what makes the second channel worth as much as
  // the first.
  function ScanFrom(lines: seq<string>, i: nat): Scan
    requires i <= |lines|
    decreases |lines| - i
    ensures forall m :: m in ScanFrom(lines, i).markers ==>
      exists k :: i <= k < |lines| && Opens(lines[k], m)
    ensures forall b :: b in ScanFrom(lines, i).malformed ==>
      && i < b.line <= |lines|
      && HasPrefix(Trim(lines[b.line - 1]), RegenTag)
      // The command too, and not only the line. Grounding that stops at "it points at a
      // REGEN line" is satisfied by a report about the right line with the wrong block in
      // it — which is exactly what a mutation replacing the `None` branch with a fabricated
      // entry did, while every assertion stayed green.
      && b.command == RegenCommand(Trim(lines[b.line - 1]))
  {
    if i == |lines| then Scan([], [])
    else
      var t := Trim(lines[i]);
      if IsTemplateLine(t) then
        var rest := ScanFrom(lines, i + 1);
        Scan([TemplateMarker(TemplateInstruction(t))] + rest.markers, rest.malformed)
      else if HasPrefix(t, RegenTag) then
        var j := if RegenOpenClosesOnItsLine(t) then i + 1 else SkipToArrow(lines, i + 1);
        var k := SkipToClose(lines, j);
        var rest := ScanFrom(lines, k);
        var here := match BlockFault(lines, i, j, k)
          case None => []
          case Some(f) => [MalformedBlock(RegenCommand(t), i + 1, f)];
        Scan([RegenMarker(RegenCommand(t), Content(lines, j, k))] + rest.markers,
             here + rest.malformed)
      else ScanFrom(lines, i + 1)
  }

  function ParseFrom(lines: seq<string>, i: nat): seq<Marker>
    requires i <= |lines|
    ensures forall m :: m in ParseFrom(lines, i) ==>
      exists k :: i <= k < |lines| && Opens(lines[k], m)
  {
    ScanFrom(lines, i).markers
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
  // The fuel on the two skips is not tuning for its own sake: without it the solver searches
  // instead of unfolding, and this lemma sat a few seconds under the 30s limit — close enough
  // that adding an unrelated lemma to the file pushed it over. A proof that passes depending
  // on what else is in the file is one that will fail on somebody else's change.
  lemma {:fuel TrimLeft, 4, 5} {:fuel TrimRight, 4, 5} {:fuel Trim, 4, 5}
        {:fuel SkipToArrow, 5, 6} {:fuel SkipToClose, 5, 6}
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
  // The marker sequence is unchanged by #524 and this lemma still holds: `parse_markers`
  // returns exactly what it always did. What changed is that the loss is no longer silent —
  // `TheSwallowedBlockIsReported` below is the same input through `ScanFrom`, and the second
  // channel names the block that took the other one.
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

  // The fault a real file carries, and the one no other lemma here reaches.
  //
  // `CloseTagMissing` needs the damaged block to be the last in the document. Give it a
  // sibling below and the scan runs past the sibling's open tag, closes on the sibling's
  // close tag, and returns one well-formed-looking block. Nothing is missing; a marker is
  // simply gone. Added because mutating this fault away left the file green — the "red" that
  // looked like a catch was an unrelated lemma timing out.
  lemma {:fuel TrimLeft, 6, 7} {:fuel TrimRight, 6, 7} {:fuel Trim, 6, 7}
        {:fuel SkipToClose, 5, 6} {:fuel BodyOpensARegenD, 5, 6}
        ABlockThatClosesOnAnothersTagIsReported()
    ensures var a := "<!-- REGEN: a -->";
            var b := "<!-- REGEN: b -->";
            ScanFrom([a, "x", b, "<!-- /REGEN -->"], 0).malformed
              == [MalformedBlock("a", 1, ClosedOnAnothersTag)]
  {
    var a := "<!-- REGEN: a -->";
    var b := "<!-- REGEN: b -->";
    var lines := [a, "x", b, "<!-- /REGEN -->"];
    assert Trim(a) == a && Trim(b) == b;
    assert Trim("x") == "x" && Trim("<!-- /REGEN -->") == CloseLine;
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
    assert Trim("x") != CloseLine by { assert |"x"| != |CloseLine|; }
    assert Trim(b) != CloseLine by { assert b[5] == 'R' && CloseLine[5] == '/'; }
    assert SkipToClose(lines, 1) == 4;
    assert HasPrefix(Trim(lines[2]), RegenTag);
    assert BodyOpensARegenD(lines, 1, 3);
    assert BlockFault(lines, 0, 1, 4) == Some(ClosedOnAnothersTag);
    assert ScanFrom(lines, 4) == Scan([], []);
  }

  // …and the same input, reported (#524).
  //
  // This is the pair that matters. `ParseMarkersIsNotComplete` proves a marker is lost;
  // this proves the loss is stated. An axiom could have asserted either one and neither
  // would have been checked against the other, which is how the incompleteness sat beside a
  // predicate declaring the opposite for as long as the file existed.
  lemma {:fuel TrimLeft, 6, 7} {:fuel TrimRight, 6, 7} {:fuel Trim, 6, 7}
        TheSwallowedBlockIsReported()
    ensures var a := "<!-- REGEN: a -->";
            var b := "<!-- REGEN: b -->";
            ScanFrom([a, b], 0).malformed == [MalformedBlock("a", 1, CloseTagMissing)]
  {
    var a := "<!-- REGEN: a -->";
    var b := "<!-- REGEN: b -->";
    var lines := [a, b];
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
    assert SkipToClose(lines, 1) == 2;
    assert BlockFault(lines, 0, 1, 2) == Some(CloseTagMissing);
    assert ScanFrom(lines, 2) == Scan([], []);
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
