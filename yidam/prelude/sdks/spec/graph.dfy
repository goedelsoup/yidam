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
// `markers.rs` transcribed: `RegenScan` reads blocks a line at a time, the way `scan_markers`
// does (#1137), and `UpdateRegen` rewrites every block the scan names, as `update_regen` does
// (#1097); `ParseFrom` walks lines the way `parse_markers` walks them. Where the scan used to
// read a block the code does not, a witness now says they agree: `ACloseTagMustStandAlone`.
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
    ensures |TrimLeft(s)| <= |s|
  { if |s| > 0 && IsSpace(s[0]) then TrimLeft(s[1..]) else s }

  function TrimRight(s: string): string
    decreases |s|
    ensures |TrimRight(s)| <= |s|
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

  // ── Lines ─────────────────────────────────────────────────────────────────────
  //
  // `scan_markers` reads a line at a time (#1137). These are the offsets it reads between:
  // where the line holding `i` ends, and where it starts. Both are offsets into the whole
  // string, which is what lets a proof talk about a line without cutting it out first.

  predicate AllSpace(x: string) { forall i :: 0 <= i < |x| ==> IsSpace(x[i]) }

  function LineEnd(s: string, i: nat): nat
    requires i <= |s|
    decreases |s| - i
    ensures i <= LineEnd(s, i) <= |s|
  { if i == |s| || s[i] == '\n' then i else LineEnd(s, i + 1) }

  function LineStart(s: string, i: nat): nat
    requires i <= |s|
    ensures LineStart(s, i) <= i
  { if i == 0 || s[i - 1] == '\n' then i else LineStart(s, i - 1) }

  // Whether the first line of `x`, and the last, hold nothing but whitespace.
  predicate FirstLineSpace(x: string) { AllSpace(x[..LineEnd(x, 0)]) }
  predicate LastLineSpace(x: string)  { AllSpace(x[LineStart(x, |x|)..]) }

  lemma LineEndFacts(s: string, i: nat)
    requires i <= |s|
    ensures LineEnd(s, i) < |s| ==> s[LineEnd(s, i)] == '\n'
    ensures forall k :: i <= k < LineEnd(s, i) ==> s[k] != '\n'
    decreases |s| - i
  {
    if !(i == |s| || s[i] == '\n') { LineEndFacts(s, i + 1); }
  }

  lemma LineEndIs(s: string, i: nat, e: nat)
    requires i <= e <= |s|
    requires e == |s| || s[e] == '\n'
    requires forall k :: i <= k < e ==> s[k] != '\n'
    ensures LineEnd(s, i) == e
    decreases e - i
  {
    if i < e { LineEndIs(s, i + 1, e); }
  }

  lemma LineEndAtLeast(s: string, i: nat, k: nat)
    requires i <= k <= |s|
    requires forall j :: i <= j < k ==> s[j] != '\n'
    ensures k <= LineEnd(s, i)
  {
    LineEndFacts(s, i);
  }

  lemma LineStartFacts(s: string, i: nat)
    requires i <= |s|
    ensures LineStart(s, i) > 0 ==> s[LineStart(s, i) - 1] == '\n'
    ensures forall k :: LineStart(s, i) <= k < i ==> s[k] != '\n'
  {
    if !(i == 0 || s[i - 1] == '\n') { LineStartFacts(s, i - 1); }
  }

  lemma LineStartIs(s: string, i: nat, b: nat)
    requires b <= i <= |s|
    requires b == 0 || s[b - 1] == '\n'
    requires forall k :: b <= k < i ==> s[k] != '\n'
    ensures LineStart(s, i) == b
    decreases i - b
  {
    if b < i { LineStartIs(s, i - 1, b); }
  }

  lemma LineEndOfSlice(s: string, a: nat, b: nat, i: nat)
    requires a + i <= b <= |s|
    ensures LineEnd(s[a..b], i) == if LineEnd(s, a + i) < b then LineEnd(s, a + i) - a else b - a
  {
    var x := s[a..b];
    var e := LineEnd(s, a + i);
    LineEndFacts(s, a + i);
    if e < b {
      assert x[e - a] == s[e];
      forall k | i <= k < e - a ensures x[k] != '\n' { assert x[k] == s[a + k]; }
      LineEndIs(x, i, e - a);
    } else {
      forall k | i <= k < b - a ensures x[k] != '\n' { assert x[k] == s[a + k]; }
      LineEndIs(x, i, b - a);
    }
  }

  lemma LineStartOfSlice(s: string, a: nat, b: nat, i: nat)
    requires a + i <= b <= |s|
    ensures LineStart(s[a..b], i)
         == if LineStart(s, a + i) >= a then LineStart(s, a + i) - a else 0
  {
    var x := s[a..b];
    var l := LineStart(s, a + i);
    LineStartFacts(s, a + i);
    if l >= a {
      if l > a { assert x[l - a - 1] == s[l - 1]; }
      forall k | l - a <= k < i ensures x[k] != '\n' { assert x[k] == s[a + k]; }
      LineStartIs(x, i, l - a);
    } else {
      forall k | 0 <= k < i ensures x[k] != '\n' { assert x[k] == s[a + k]; }
      LineStartIs(x, i, 0);
    }
  }

  // The first line of a suffix is the rest of the line it was cut from.
  lemma FirstLineOfSuffix(s: string, k: nat)
    requires k <= |s|
    ensures FirstLineSpace(s[k..]) == AllSpace(s[k..LineEnd(s, k)])
  {
    LineEndOfSlice(s, k, |s|, 0);
    assert s[k..] == s[k..|s|];
    assert LineEnd(s[k..], 0) == LineEnd(s, k) - k;
    assert s[k..][..LineEnd(s, k) - k] == s[k..LineEnd(s, k)];
  }

  // ── Trim, taken apart ─────────────────────────────────────────────────────────

  lemma TrimShrinks(x: string)
    ensures |TrimLeft(x)| <= |x| && |TrimRight(x)| <= |x|
    decreases |x|
  {
    if |x| > 0 {
      TrimShrinks(x[1..]);
      TrimShrinks(x[..|x| - 1]);
    }
  }

  lemma TrimRightIsPrefix(x: string)
    ensures |TrimRight(x)| <= |x|
    ensures TrimRight(x) == x[..|TrimRight(x)|]
    ensures AllSpace(x[|TrimRight(x)|..])
    ensures |TrimRight(x)| > 0 ==> !IsSpace(x[|TrimRight(x)| - 1])
    decreases |x|
  {
    if |x| > 0 && IsSpace(x[|x| - 1]) {
      var y := x[..|x| - 1];
      TrimRightIsPrefix(y);
      var n := |TrimRight(y)|;
      assert y[..n] == x[..n];
      assert x[n..] == y[n..] + [x[|x| - 1]];
      assert n > 0 ==> x[n - 1] == y[n - 1];
    }
  }

  lemma TrimLeftIsSuffix(x: string)
    ensures |TrimLeft(x)| <= |x|
    ensures TrimLeft(x) == x[|x| - |TrimLeft(x)|..]
    ensures AllSpace(x[..|x| - |TrimLeft(x)|])
    decreases |x|
  {
    if |x| > 0 && IsSpace(x[0]) {
      var y := x[1..];
      TrimLeftIsSuffix(y);
      var n := |TrimLeft(y)|;
      assert y[|y| - n..] == x[|x| - n..];
      assert x[..|x| - n] == [x[0]] + y[..|y| - n];
    }
  }

  lemma TrimRightSpaceTail(x: string, w: string)
    requires AllSpace(w)
    ensures TrimRight(x + w) == TrimRight(x)
    decreases |w|
  {
    if |w| == 0 {
      assert x + w == x;
    } else {
      var w' := w[..|w| - 1];
      assert (x + w)[..|x + w| - 1] == x + w';
      TrimRightSpaceTail(x, w');
    }
  }

  lemma TrimLeftSpaceHead(w: string, x: string)
    requires AllSpace(w)
    ensures TrimLeft(w + x) == TrimLeft(x)
    decreases |w|
  {
    if |w| == 0 {
      assert w + x == x;
    } else {
      assert (w + x)[1..] == w[1..] + x;
      TrimLeftSpaceHead(w[1..], x);
    }
  }

  lemma TagShapes()
    ensures forall i :: 0 <= i < |RegenClose| ==> RegenClose[i] != '\n'
    ensures forall i :: 0 <= i < |RegenOpen|  ==> RegenOpen[i]  != '\n'
    ensures forall i :: 0 <= i < |RegenTag|   ==> RegenTag[i]   != '\n'
    ensures |RegenTag| == 11 && RegenTag[0] == '<' && !IsSpace(RegenTag[10])
    ensures |RegenArrow| == 3 && !IsSpace(RegenArrow[2])
    ensures |RegenClose| == 15 && RegenClose[0] == '<' && RegenClose[14] == '>'
  {}

  // A close tag with nothing but whitespace around it trims to the close tag, and starts
  // where the whitespace in front of it ends.
  lemma PaddedClose(w1: string, w2: string)
    requires AllSpace(w1) && AllSpace(w2)
    ensures Trim(w1 + RegenClose + w2) == RegenClose
    ensures |TrimRight(w1 + RegenClose + w2)| == |w1| + |RegenClose|
  {
    TagShapes();
    assert w1 + RegenClose + w2 == (w1 + RegenClose) + w2;
    TrimRightSpaceTail(w1 + RegenClose, w2);
    assert (w1 + RegenClose)[|w1 + RegenClose| - 1] == '>';
    TrimLeftSpaceHead(w1, RegenClose);
  }

  // …and a line that trims to the close tag is one of those.
  lemma TrimIsClose(line: string)
    requires Trim(line) == RegenClose
    ensures var n := |TrimRight(line)|;
      && |RegenClose| <= n <= |line|
      && SubstringAt(line, RegenClose, n - |RegenClose|)
      && AllSpace(line[..n - |RegenClose|])
      && AllSpace(line[n..])
  {
    TrimRightIsPrefix(line);
    var tr := TrimRight(line);
    var n := |tr|;
    TrimLeftIsSuffix(tr);
    assert line[n - 15..n] == tr[n - 15..];
    assert line[..n - 15] == tr[..n - 15];
  }

  // ── Finding a tag on a line ───────────────────────────────────────────────────

  lemma FindHere(s: string, sub: string, i: nat)
    requires SubstringAt(s, sub, i)
    ensures FindFrom(s, sub, i) == Some(i)
  {}

  // Trailing whitespace hides no tag: each one ends in a character that is not a space, so
  // no occurrence can end inside the whitespace.
  lemma FindIgnoresSpaceTail(x: string, w: string, sub: string, from: nat)
    requires |sub| > 0 && !IsSpace(sub[|sub| - 1])
    requires AllSpace(w) && from <= |x|
    ensures FindFrom(x + w, sub, from) == FindFrom(x, sub, from)
    decreases |x| - from
  {
    var y := x + w;
    if from + |sub| > |x| {
      if FindFrom(y, sub, from).Some? {
        var v := FindFrom(y, sub, from).value;
        assert y[v..v + |sub|][|sub| - 1] == sub[|sub| - 1];
        assert y[v + |sub| - 1] == w[v + |sub| - 1 - |x|];
      }
    } else {
      assert y[from..from + |sub|] == x[from..from + |sub|];
      if x[from..from + |sub|] != sub {
        FindIgnoresSpaceTail(x, w, sub, from + 1);
      }
    }
  }

  lemma FindIgnoresSpaceTailOf(x: string, w: string, sub: string, from: nat)
    requires |sub| > 0 && !IsSpace(sub[|sub| - 1])
    requires AllSpace(w) && from <= |x| + |w|
    ensures from <= |x| ==> FindFrom(x + w, sub, from) == FindFrom(x, sub, from)
    ensures from > |x| ==> FindFrom(x + w, sub, from).None?
  {
    if from <= |x| {
      FindIgnoresSpaceTail(x, w, sub, from);
    } else if FindFrom(x + w, sub, from).Some? {
      var v := FindFrom(x + w, sub, from).value;
      assert (x + w)[v..v + |sub|][|sub| - 1] == sub[|sub| - 1];
      assert (x + w)[v + |sub| - 1] == w[v + |sub| - 1 - |x|];
    }
  }

  // ── The scan ──────────────────────────────────────────────────────────────────
  //
  // `scan_markers` reads a line at a time, and so does this (#1137). On each line it looks
  // for blocks that open *and close* there — the inline form, `find` three times — and a
  // line may hold more than one. An open tag with no close beside it is the block form,
  // which is a block only where the tag starts its line: in the middle of a sentence it is
  // prose, and the scan goes on to the next line. A block-form open tag ends on the first
  // line that ends in `-->`, and its close tag is the first line after that which is the
  // close tag and nothing else.
  //
  // `m` says whether `s` starts in the middle of a line — after a block the scan has just
  // read. Then what went before on that line is the block, and an open tag there does not
  // start its line.

  datatype LineRead = NoBlock | InlineBlock(sp: Span) | OpensBlock(o: nat)

  // An open tag whose line has no close tag after it.
  function OpenOrProse(line: string, o: nat, m: bool): LineRead
    requires o <= |line|
  { if !m && AllSpace(line[..o]) then OpensBlock(o) else NoBlock }

  // What the first open tag on a line is. The tag is `RegenTag`, without the space, because
  // that is what `scan_markers` searches for.
  function ReadLine(line: string, m: bool): LineRead
    ensures ReadLine(line, m).InlineBlock? ==>
      var sp := ReadLine(line, m).sp;
      && WellPlaced(line, sp)
      && FindFrom(line, RegenTag, 0) == Some(sp.open)
      && FindFrom(line, RegenArrow, sp.open + |RegenTag|) == Some(sp.arrow)
      && FindFrom(line, RegenClose, sp.body) == Some(sp.close)
    ensures ReadLine(line, m).OpensBlock? ==>
      var o := ReadLine(line, m).o;
      && o + |RegenTag| <= |line|
      && FindFrom(line, RegenTag, 0) == Some(o)
      && !m && AllSpace(line[..o])
  {
    match FindFrom(line, RegenTag, 0)
      case None => NoBlock
      case Some(o) =>
        match FindFrom(line, RegenArrow, o + |RegenTag|)
          case None => OpenOrProse(line, o, m)
          case Some(a) =>
            match FindFrom(line, RegenClose, a + |RegenArrow|)
              case None => OpenOrProse(line, o, m)
              case Some(c) => InlineBlock(Span(o, a, a + |RegenArrow|, c))
  }

  // The first line at or after `from` that ends in `-->` once its trailing whitespace is
  // gone, and where the body starts: after the arrow, before that whitespace.
  function FindArrowLine(s: string, from: nat): Option<nat>
    requires from <= |s|
    decreases |s| - from
    ensures FindArrowLine(s, from).Some? ==>
      from + |RegenArrow| <= FindArrowLine(s, from).value <= |s|
  {
    var e := LineEnd(s, from);
    var t := TrimRight(s[from..e]);
    if HasSuffix(t, RegenArrow) then Some(from + |t|)
    else if e == |s| then None
    else FindArrowLine(s, e + 1)
  }

  // The first line at or after `from` that is the close tag and nothing else, and where on
  // it the tag starts.
  function FindCloseLine(s: string, from: nat): Option<nat>
    requires from <= |s|
    decreases |s| - from
    ensures FindCloseLine(s, from).Some? ==>
      from <= FindCloseLine(s, from).value
      && FindCloseLine(s, from).value + |RegenClose| <= |s|
  {
    var e := LineEnd(s, from);
    var line := s[from..e];
    if Trim(line) == RegenClose then Some(from + |TrimRight(line)| - |RegenClose|)
    else if e == |s| then None
    else FindCloseLine(s, e + 1)
  }

  // The close tag of a block-form block whose open tag is at `o`, whose body starts at `b`,
  // and whose arrow's line ends at `e`: on a later line than that.
  function CloseAfter(s: string, o: nat, b: nat, e: nat): Option<Span>
    requires o + |RegenTag| + |RegenArrow| <= b <= e <= |s|
    ensures CloseAfter(s, o, b, e).Some? ==>
      var sp := CloseAfter(s, o, b, e).value;
      WellPlaced(s, sp) && sp.open == o && sp.body == b && e < sp.close
  {
    if e == |s| then None
    else match FindCloseLine(s, e + 1)
      case None => None
      case Some(c) => Some(Span(o, b - |RegenArrow|, b, c))
  }

  // The block form, from an open tag at `o` on a line that ends at `e`. The tag ends on its
  // own line when the rest of that line ends in `-->`; otherwise the command is the rest of
  // the line, and the tag ends on the first line below that ends in `-->`.
  function BlockForm(s: string, o: nat, e: nat): Option<Span>
    requires o + |RegenTag| <= e <= |s|
  {
    var t := TrimRight(s[o + |RegenTag|..e]);
    if HasSuffix(t, RegenArrow) then CloseAfter(s, o, o + |RegenTag| + |t|, e)
    else if e == |s| then None
    else match FindArrowLine(s, e + 1)
      case None => None
      case Some(b) => CloseAfter(s, o, b, LineEnd(s, b))
  }

  // The first block in `s`. A line with none sends the scan to the next line, which starts
  // a line; a block form that does not close ends the scan, as a fault does in the code.
  function NextBlock(s: string, m: bool): Option<Span>
    decreases |s|
    ensures NextBlock(s, m).Some? ==> WellPlaced(s, NextBlock(s, m).value)
  {
    var e := LineEnd(s, 0);
    match ReadLine(s[..e], m)
      case InlineBlock(sp) => Some(sp)
      case OpensBlock(o) => BlockForm(s, o, e)
      case NoBlock =>
        if e == |s| then None
        else match NextBlock(s[e + 1..], false)
          case None => None
          case Some(sp) => Some(Shift(sp, e + 1))
  }

  // The command: what follows the open tag, up to the arrow or the end of the tag's line,
  // whichever is first, trimmed. A function of the text before the arrow alone.
  function CommandAt(s: string, sp: Span): string
    requires WellPlaced(s, sp)
  {
    var h := s[..sp.arrow];
    Trim(h[sp.open + |RegenTag|..LineEnd(h, sp.open + |RegenTag|)])
  }

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
  // close tag, which is where the scan resumes — a body is never searched for an open tag —
  // and it resumes in the middle of that close tag's line. `base` is how far into the original
  // text `s` starts, so the spans are the original's.
  function ScanAt(s: string, m: bool, base: nat): seq<Block>
    decreases |s|
  {
    match NextBlock(s, m)
      case None => []
      case Some(sp) =>
        [Block(CommandAt(s, sp), Shift(sp, base), InlineAt(s, sp))]
          + ScanAt(s[End(sp)..], true, base + End(sp))
  }

  // Every span the scan reports is where it says, in the string it read. A lemma rather than
  // a postcondition of `ScanAt`: as a postcondition it is a quantifier every proof that
  // mentions a scan pays for, and the inductions below time out paying it.
  lemma ScanIsPlaced(s: string, m: bool, base: nat)
    ensures forall b :: b in ScanAt(s, m, base) ==> PlacedAt(s, b.span, base)
    decreases |s|
  {
    if NextBlock(s, m).Some? {
      var sp := NextBlock(s, m).value;
      ScanIsPlaced(s[End(sp)..], true, base + End(sp));
    }
  }

  // One step of the scan, as a fact a proof can call on. Left to the solver to unfold inside
  // a larger proof, this equation alone runs it out of time.
  lemma ScanAtSteps(s: string, m: bool, base: nat, sp: Span)
    requires NextBlock(s, m) == Some(sp)
    ensures ScanAt(s, m, base)
         == [Block(CommandAt(s, sp), Shift(sp, base), InlineAt(s, sp))]
            + ScanAt(s[End(sp)..], true, base + End(sp))
  {}

  function RegenScan(text: string): seq<Block> { ScanAt(text, false, 0) }


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
    ScanIsPlaced(text, false, 0);
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

  // ── Which blocks are written ──────────────────────────────────────────────────
  //
  // Block form is a *line* form (#1137): the scan reads one only where the open tag starts
  // its line and the close tag stands alone on its own. So a multi-line value can go into an
  // inline block only where nothing else shares the block's line. Anywhere else the block is
  // left as it is. In the middle of a sentence the open tag would become prose and the block
  // would never be read again. With text after the close tag, the next run would read a block
  // that runs on to some later block's close tag.

  // Nothing but whitespace before `o` on its line. With `m`, `s` starts mid-line, so a line
  // that starts where `s` does started before it, on text that is not whitespace.
  predicate OpensALine(s: string, m: bool, o: nat)
    requires o <= |s|
  { AllSpace(s[LineStart(s, o)..o]) && (LineStart(s, o) > 0 || !m) }

  // `stands_alone` in `markers.rs`: nothing but whitespace shares the block's line.
  predicate StandsAlone(s: string, m: bool, sp: Span)
    requires WellPlaced(s, sp)
  { OpensALine(s, m, sp.open) && FirstLineSpace(s[End(sp)..]) }

  // Whether the block can take `newContent` at all: `regen_body` returning `Some`.
  predicate Holds(s: string, m: bool, sp: Span, newContent: string)
    requires WellPlaced(s, sp)
  { !InlineAt(s, sp) || NoNewline(newContent) || StandsAlone(s, m, sp) }

  predicate Writes(s: string, m: bool, sp: Span, command: string, newContent: string)
    requires WellPlaced(s, sp)
  { CommandAt(s, sp) == command && Holds(s, m, sp, newContent) }

  // The body a block named `command` holds after the rewrite.
  function Written(s: string, m: bool, sp: Span, newContent: string): string
    requires WellPlaced(s, sp)
  {
    if Holds(s, m, sp, newContent) then RegenBody(newContent, InlineAt(s, sp))
    else s[sp.body..sp.close]
  }

  // What goes between a block's arrow and its close tag: the new body if the block is named
  // `command` and can hold it, and what was there if not.
  function Mid(s: string, m: bool, sp: Span, command: string, newContent: string): string
    requires WellPlaced(s, sp)
  {
    if Writes(s, m, sp, command, newContent) then RegenBody(newContent, InlineAt(s, sp))
    else s[sp.body..sp.close]
  }

  // `update_regen`: every block the scan reads with the command, rewritten — not the first.
  function Rewrite(s: string, m: bool, command: string, newContent: string): string
    decreases |s|
  {
    match NextBlock(s, m)
      case None => s
      case Some(sp) =>
        s[..sp.body] + Mid(s, m, sp, command, newContent) + s[sp.close..End(sp)]
          + Rewrite(s[End(sp)..], true, command, newContent)
  }

  function UpdateRegen(text: string, command: string, newContent: string): string {
    Rewrite(text, false, command, newContent)
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

  // ── What the scan reads, as facts ─────────────────────────────────────────────

  lemma TrimRightStops(x: string)
    requires |x| > 0 && !IsSpace(x[|x| - 1])
    ensures TrimRight(x) == x
  {}

  lemma SliceOfSlice(s: string, a: nat, b: nat, i: nat, j: nat)
    requires a <= b <= |s| && i <= j <= b - a
    ensures s[a..b][i..j] == s[a + i..a + j]
  {
    assert |s[a..b][i..j]| == |s[a + i..a + j]|;
    forall k | 0 <= k < j - i ensures s[a..b][i..j][k] == s[a + i..a + j][k] {
      assert s[a..b][i..j][k] == s[a..b][i + k] == s[a + i + k];
    }
  }

  lemma SubstringOfPrefix(s: string, sub: string, cut: nat, j: nat)
    requires cut <= |s| && SubstringAt(s[..cut], sub, j)
    ensures SubstringAt(s, sub, j)
  {
    assert s[..cut][j..j + |sub|] == s[j..j + |sub|];
  }

  // Where `FindCloseLine` lands: the close tag, with only whitespace after it on its line.
  lemma FindCloseLineFacts(s: string, from: nat)
    requires from <= |s|
    requires FindCloseLine(s, from).Some?
    ensures var c := FindCloseLine(s, from).value;
      && SubstringAt(s, RegenClose, c)
      && AllSpace(s[c + |RegenClose|..LineEnd(s, c + |RegenClose|)])
    decreases |s| - from
  {
    var e := LineEnd(s, from);
    var line := s[from..e];
    if Trim(line) == RegenClose {
      TrimIsClose(line);
      var n := |TrimRight(line)|;
      var c := from + n - |RegenClose|;
      SliceOfSlice(s, from, e, n - |RegenClose|, n);
      SliceOfSlice(s, from, e, n, e - from);
      LineEndFacts(s, from);
      LineEndIs(s, c + |RegenClose|, e);

    } else {
      FindCloseLineFacts(s, e + 1);
    }
  }

  // Where `CloseAfter` lands, and that the body it gives is block form: it holds the newline
  // that ends the arrow's line.
  lemma CloseAfterFacts(s: string, o: nat, b: nat, e: nat)
    requires o + |RegenTag| + |RegenArrow| <= b <= e <= |s|
    requires e == |s| || s[e] == '\n'
    requires CloseAfter(s, o, b, e).Some?
    ensures var sp := CloseAfter(s, o, b, e).value;
      && SubstringAt(s, RegenClose, sp.close)
      && FirstLineSpace(s[End(sp)..])
      && !InlineAt(s, sp)
  {
    var sp := CloseAfter(s, o, b, e).value;
    FindCloseLineFacts(s, e + 1);
    FirstLineOfSuffix(s, End(sp));
    assert s[sp.body..sp.close][e - sp.body] == '\n';
  }

  lemma BlockFormFacts(s: string, o: nat, e: nat)
    requires o + |RegenTag| <= e <= |s|
    requires e == |s| || s[e] == '\n'
    requires BlockForm(s, o, e).Some?
    ensures var sp := BlockForm(s, o, e).value;
      && sp.open == o
      && SubstringAt(s, RegenClose, sp.close)
      && FirstLineSpace(s[End(sp)..])
      && !InlineAt(s, sp)
  {
    var t := TrimRight(s[o + |RegenTag|..e]);
    if HasSuffix(t, RegenArrow) {
      CloseAfterFacts(s, o, o + |RegenTag| + |t|, e);
    } else {
      var b := FindArrowLine(s, e + 1).value;
      LineEndFacts(s, b);
      CloseAfterFacts(s, o, b, LineEnd(s, b));
    }
  }

  lemma ShiftedFacts(s: string, e: nat, sp: Span)
    requires e + 1 <= |s| && WellPlaced(s[e + 1..], sp)
    ensures var d := e + 1;
      && (SubstringAt(s[d..], RegenTag, sp.open) ==> SubstringAt(s, RegenTag, sp.open + d))
      && (SubstringAt(s[d..], RegenClose, sp.close) ==> SubstringAt(s, RegenClose, sp.close + d))
      && InlineAt(s, Shift(sp, d)) == InlineAt(s[d..], sp)
      && s[End(Shift(sp, d))..] == s[d..][End(sp)..]
  {
    var d := e + 1;
    assert s[d..][sp.open..sp.open + |RegenTag|] == s[sp.open + d..sp.open + d + |RegenTag|];
    assert s[d..][sp.close..End(sp)] == s[sp.close + d..End(sp) + d];
    assert s[d..][sp.body..sp.close] == s[sp.body + d..sp.close + d];
  }

  // Every block the scan reads starts with the open tag and ends with the close tag, and a
  // block-form block's close tag stands alone to the end of its line.
  lemma NextBlockFacts(s: string, m: bool)
    requires NextBlock(s, m).Some?
    ensures var sp := NextBlock(s, m).value;
      && SubstringAt(s, RegenTag, sp.open)
      && SubstringAt(s, RegenClose, sp.close)
      && (!InlineAt(s, sp) ==> FirstLineSpace(s[End(sp)..]))
    decreases |s|
  {
    var e := LineEnd(s, 0);
    LineEndFacts(s, 0);
    match ReadLine(s[..e], m)
      case InlineBlock(sp) =>
        SubstringOfPrefix(s, RegenTag, e, sp.open);
        SubstringOfPrefix(s, RegenClose, e, sp.close);
      case OpensBlock(o) =>
        SubstringOfPrefix(s, RegenTag, e, o);
        BlockFormFacts(s, o, e);
      case NoBlock =>
        var sp0 := NextBlock(s[e + 1..], false).value;
        NextBlockFacts(s[e + 1..], false);
        ShiftedFacts(s, e, sp0);
  }

  // ── Reading a line again ──────────────────────────────────────────────────────

  lemma SliceSplit(s: string, a: nat, b: nat, c: nat)
    requires a <= b <= c <= |s|
    ensures s[a..c] == s[a..b] + s[b..c]
  {}

  // Trailing whitespace changes nothing a line is read as: every tag ends in a character
  // that is not a space, so none can end inside it.
  lemma ReadLineIgnoresTrailingSpace(x: string, w: string, m: bool)
    requires AllSpace(w)
    ensures ReadLine(x + w, m) == ReadLine(x, m)
  {
    TagShapes();
    var y := x + w;
    FindIgnoresSpaceTail(x, w, RegenTag, 0);
    if FindFrom(x, RegenTag, 0).Some? {
      var o := FindFrom(x, RegenTag, 0).value;
      FindIgnoresSpaceTail(x, w, RegenArrow, o + |RegenTag|);
      assert y[..o] == x[..o];
      if FindFrom(x, RegenArrow, o + |RegenTag|).Some? {
        var a := FindFrom(x, RegenArrow, o + |RegenTag|).value;
        FindIgnoresSpaceTail(x, w, RegenClose, a + |RegenArrow|);
      }
    }
  }

  // The close tag of a block-form block is found on the first line that is the close tag and
  // nothing else — provided no close tag stands on any line before it.
  lemma CloseLineFinds(r: string, from: nat, c: nat)
    requires SubstringAt(r, RegenClose, c)
    requires from <= LineStart(r, c)
    requires AllSpace(r[LineStart(r, c)..c])
    requires AllSpace(r[c + |RegenClose|..LineEnd(r, c + |RegenClose|)])
    requires forall j :: from <= j < LineStart(r, c) ==> !SubstringAt(r, RegenClose, j)
    ensures FindCloseLine(r, from) == Some(c)
    decreases |r| - from
  {
    TagShapes();
    var ls := LineStart(r, c);
    LineStartFacts(r, c);
    var e := LineEnd(r, from);
    LineEndFacts(r, from);
    if from == ls {
      var ce := LineEnd(r, c + |RegenClose|);
      LineEndFacts(r, c + |RegenClose|);
      forall k | from <= k < ce ensures r[k] != '\n' {
        if c <= k < c + |RegenClose| { assert r[k] == r[c..c + |RegenClose|][k - c]; }
      }
      LineEndIs(r, from, ce);
      var w1 := r[ls..c];
      var w2 := r[c + |RegenClose|..ce];
      SliceSplit(r, from, c, e);
      SliceSplit(r, c, c + |RegenClose|, e);
      assert r[c..c + |RegenClose|] == RegenClose;
      assert r[from..e] == w1 + RegenClose + w2;
      PaddedClose(w1, w2);
      assert |TrimRight(r[from..e])| == |w1| + |RegenClose|;
      assert Trim(r[from..e]) == RegenClose;
    } else {
      assert r[ls - 1] == '\n';
      assert e <= ls - 1;
      var line := r[from..e];
      if Trim(line) == RegenClose {
        TrimIsClose(line);
        var n := |TrimRight(line)|;
        SliceOfSlice(r, from, e, n - |RegenClose|, n);
        assert SubstringAt(r, RegenClose, from + n - |RegenClose|);
        assert false;
      }
      CloseLineFinds(r, e + 1, c);
    }
  }

  lemma FindArrowLineSteps(s: string, from: nat)
    requires from <= |s|
    ensures var e := LineEnd(s, from);
      var t := TrimRight(s[from..e]);
      FindArrowLine(s, from)
        == if HasSuffix(t, RegenArrow) then Some(from + |t|)
           else if e == |s| then None
           else FindArrowLine(s, e + 1)
  {}

  // Two texts that agree up to the end of the arrow, and whose arrow's line holds nothing
  // after it but whitespace in both, find the arrow on the same line.
  lemma {:isolate_assertions} ArrowLineShared(s: string, r: string, from: nat, b: nat)
    requires from <= |s| && from <= |r|
    requires FindArrowLine(s, from) == Some(b)
    requires b <= |r| && r[..b] == s[..b]
    requires AllSpace(r[b..LineEnd(r, b)])
    ensures FindArrowLine(r, from) == Some(b)
    decreases |s| - from
  {
    hide FindArrowLine;
    FindArrowLineSteps(s, from);
    FindArrowLineSteps(r, from);
    TagShapes();
    var e := LineEnd(s, from);
    LineEndFacts(s, from);
    var t := TrimRight(s[from..e]);
    if HasSuffix(t, RegenArrow) {
      TrimRightIsPrefix(s[from..e]);
      assert b == from + |t|;
      SliceOfSlice(s, from, e, 0, |t|);
      assert s[from..b] == t;
      assert r[from..b] == t by { assert r[from..b] == r[..b][from..b]; assert s[from..b] == s[..b][from..b]; }
      var re := LineEnd(r, b);
      LineEndFacts(r, b);
      forall k | from <= k < re ensures r[k] != '\n' {
        if k < b { assert r[k] == r[..b][k] == s[..b][k] == s[k]; }
      }
      LineEndIs(r, from, re);
      assert r[from..re] == t + r[b..re];
      TrimRightSpaceTail(t, r[b..re]);
      assert t[|t| - 1] == RegenArrow[2] by { assert t[|t| - 3..][2] == RegenArrow[2]; }
      TrimRightStops(t);
      assert TrimRight(r[from..re]) == t;
      assert FindArrowLine(r, from) == Some(from + |t|);
    } else {
      assert e < |s|;
      assert FindArrowLine(s, e + 1) == Some(b);
      assert e + 1 + |RegenArrow| <= b;
      assert r[..e + 1] == s[..e + 1] by { assert r[..e + 1] == r[..b][..e + 1]; assert s[..e + 1] == s[..b][..e + 1]; }
      forall k | from <= k < e ensures r[k] != '\n' { assert r[k] == r[..e + 1][k]; }
      assert r[e] == r[..e + 1][e];
      LineEndIs(r, from, e);
      assert r[from..e] == s[from..e] by { assert r[from..e] == r[..e + 1][from..e]; assert s[from..e] == s[..e + 1][from..e]; }
      ArrowLineShared(s, r, e + 1, b);
      assert FindArrowLine(r, from) == FindArrowLine(r, e + 1);
    }
  }

  // The same for the close tag.
  lemma CloseLineShared(s: string, r: string, from: nat, c: nat)
    requires from <= |s| && from <= |r|
    requires FindCloseLine(s, from) == Some(c)
    requires c + |RegenClose| <= |r| && r[..c + |RegenClose|] == s[..c + |RegenClose|]
    requires AllSpace(r[c + |RegenClose|..LineEnd(r, c + |RegenClose|)])
    ensures FindCloseLine(r, from) == Some(c)
    decreases |s| - from
  {
    TagShapes();
    var e := LineEnd(s, from);
    LineEndFacts(s, from);
    var line := s[from..e];
    var ce := c + |RegenClose|;
    if Trim(line) == RegenClose {
      TrimIsClose(line);
      var n := |TrimRight(line)|;
      assert c == from + n - |RegenClose|;
      SliceOfSlice(s, from, e, 0, n);
      SliceOfSlice(s, from, e, 0, n - |RegenClose|);
      SliceOfSharedPrefix(s, r, from, ce, ce);
      SliceOfSharedPrefix(s, r, from, c, ce);
      var re := LineEnd(r, ce);
      LineEndFacts(r, ce);
      forall k | from <= k < re ensures r[k] != '\n' {
        if k < ce { SliceOfSharedPrefix(s, r, k, k + 1, ce); assert s[k..k + 1][0] == s[k]; assert r[k..k + 1][0] == r[k]; }
      }
      LineEndIs(r, from, re);
      var w1 := r[from..c];
      assert AllSpace(w1);
      SliceSplit(r, from, c, re);
      SliceSplit(r, c, ce, re);
      assert r[from..re] == w1 + RegenClose + r[ce..re];
      PaddedClose(w1, r[ce..re]);
      assert |w1| == c - from;
      assert FindCloseLine(r, from) == Some(from + |TrimRight(r[from..re])| - |RegenClose|);
    } else {
      assert e < |s|;
      assert e + 1 <= c;
      SliceOfSharedPrefix(s, r, from, e + 1, ce);
      assert s[from..e + 1][e - from] == s[e];
      assert r[from..e + 1][e - from] == r[e];
      forall k | from <= k < e ensures r[k] != '\n' {
        assert s[from..e + 1][k - from] == s[k]; assert r[from..e + 1][k - from] == r[k];
      }
      LineEndIs(r, from, e);
      SliceOfSlice(s, from, e + 1, 0, e - from);
      SliceOfSlice(r, from, e + 1, 0, e - from);
      assert r[from..e] == line;
      CloseLineShared(s, r, e + 1, c);
      assert FindCloseLine(r, from) == FindCloseLine(r, e + 1);
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
  //
  // Under a line-anchored scan that is true only of a body that fits the lines around it:
  // a body on one line in a block that was inline, or a body that starts and ends its own
  // lines, with the close tag after it standing alone. `Fits` is that condition, and a body
  // `update_regen` writes meets it — `MidFits`.

  // `s` with the body of `sp` replaced by `mid`, and `tail` after the close tag.
  function Put(s: string, sp: Span, mid: string, tail: string): string
    requires WellPlaced(s, sp)
  { s[..sp.body] + mid + s[sp.close..End(sp)] + tail }

  predicate NoCloseIn(r: string, a: nat, b: nat) {
    forall j :: a <= j < b ==> !SubstringAt(r, RegenClose, j)
  }

  predicate Fits(s: string, m: bool, sp: Span, mid: string, tail: string)
    requires WellPlaced(s, sp)
  {
    if NoNewline(mid) then InlineAt(s, sp)
    else
      && FirstLineSpace(mid) && LastLineSpace(mid) && FirstLineSpace(tail)
      && (InlineAt(s, sp) ==> OpensALine(s, m, sp.open))
  }

  lemma NewlineIn(mid: string)
    requires !NoNewline(mid)
    ensures LineEnd(mid, 0) < |mid|
    ensures LineEnd(mid, 0) < LineStart(mid, |mid|)
    ensures LineStart(mid, |mid|) > 0 && mid[LineStart(mid, |mid|) - 1] == '\n'
  {
    LineEndFacts(mid, 0);
    LineStartFacts(mid, |mid|);
    var k := LineEnd(mid, 0);
    if k == |mid| { assert NoNewline(mid); }
  }

  // A body that starts and ends its own lines, read back as block form: the close tag is on
  // the first line after the arrow's that is the close tag alone.
  lemma ReadBackClose(r: string, o: nat, b: nat, mid: string, tail: string)
    requires o + |RegenTag| + |RegenArrow| <= b
    requires b + |mid| + |RegenClose| + |tail| == |r|
    requires r[b..b + |mid|] == mid
    requires SubstringAt(r, RegenClose, b + |mid|)
    requires r[b + |mid| + |RegenClose|..] == tail
    requires !NoNewline(mid) && FirstLineSpace(mid) && LastLineSpace(mid) && FirstLineSpace(tail)
    requires NoCloseIn(r, b, b + |mid|)
    ensures LineEnd(r, b) == b + LineEnd(mid, 0) < |r|
    ensures AllSpace(r[b..LineEnd(r, b)])
    ensures CloseAfter(r, o, b, LineEnd(r, b)) == Some(Span(o, b - |RegenArrow|, b, b + |mid|))
  {
    NewlineIn(mid);
    LineEndFacts(mid, 0);
    LineStartFacts(mid, |mid|);
    var k := LineEnd(mid, 0);
    var c := b + |mid|;
    forall i | b <= i < c ensures r[i] == mid[i - b] {
      assert r[b..b + |mid|][i - b] == r[i];
    }
    LineEndIs(r, b, b + k);
    assert r[b..b + k] == mid[..k];
    var ls := LineStart(mid, |mid|);
    LineStartIs(r, c, b + ls);
    assert r[b + ls..c] == mid[ls..];
    assert r[c + |RegenClose|..] == tail;
    FirstLineOfSuffix(r, c + |RegenClose|);
    CloseLineFinds(r, b + k + 1, c);
  }

  // A block that was inline, given a body with no newline, is read inline again.
  lemma ReadBackInline(s: string, m: bool, sp: Span, mid: string, tail: string)
    requires WellPlaced(s, sp)
    requires ReadLine(s[..LineEnd(s, 0)], m) == InlineBlock(sp)
    requires NoNewline(mid)
    requires SubstringAt(s, RegenClose, sp.close)
    requires NoCloseIn(Put(s, sp, mid, tail), sp.body, sp.body + |mid|)
    ensures NextBlock(Put(s, sp, mid, tail), m)
         == Some(Span(sp.open, sp.arrow, sp.body, sp.body + |mid|))
  {
    TagShapes();
    var e := LineEnd(s, 0);
    LineEndFacts(s, 0);
    var x := s[..e];
    var r := Put(s, sp, mid, tail);
    var sp' := Span(sp.open, sp.arrow, sp.body, sp.body + |mid|);
    var c' := sp.body + |mid|;
    assert r[..sp.body] == s[..sp.body];
    assert r[c'..End(sp')] == RegenClose;
    forall i | 0 <= i < End(sp') ensures r[i] != '\n' {
      if i < sp.body {
        assert r[i] == r[..sp.body][i];
      } else if i < c' {
        assert r[i] == mid[i - sp.body];
      } else {
        assert r[i] == r[c'..End(sp')][i - c'];
      }
    }
    LineEndAtLeast(r, 0, End(sp'));
    var x' := r[..LineEnd(r, 0)];
    assert x'[..sp.body] == x[..sp.body];
    FindAgreesOnSharedPrefix(x, x', RegenTag, 0, sp.body);
    FindAgreesOnSharedPrefix(x, x', RegenArrow, sp.open + |RegenTag|, sp.body);
    forall j | sp.body <= j < c' ensures !SubstringAt(x', RegenClose, j) {
      if SubstringAt(x', RegenClose, j) { SubstringOfPrefix(r, RegenClose, LineEnd(r, 0), j); }
    }
    FindSkips(x', RegenClose, sp.body, c');
    assert x'[c'..End(sp')] == r[c'..End(sp')];
    FindHere(x', RegenClose, c');
    assert ReadLine(x', m) == InlineBlock(sp');
  }

  // A block whose arrow ends its line's text, given a body that starts a new line: block form.
  lemma ReadBackBlockSameLine(s: string, m: bool, sp: Span, mid: string, tail: string)
    requires WellPlaced(s, sp)
    requires ReadLine(s[..sp.body], m) == OpensBlock(sp.open)
    requires SubstringAt(s, RegenArrow, sp.arrow)
    requires SubstringAt(s, RegenClose, sp.close)
    requires forall i :: 0 <= i < sp.body ==> s[i] != '\n'
    requires !NoNewline(mid) && FirstLineSpace(mid) && LastLineSpace(mid) && FirstLineSpace(tail)
    requires NoCloseIn(Put(s, sp, mid, tail), sp.body, sp.body + |mid|)
    ensures NextBlock(Put(s, sp, mid, tail), m)
         == Some(Span(sp.open, sp.arrow, sp.body, sp.body + |mid|))
  {
    TagShapes();
    var r := Put(s, sp, mid, tail);
    var b := sp.body;
    assert r[b..b + |mid|] == mid;
    assert r[b + |mid|..b + |mid| + |RegenClose|] == s[sp.close..End(sp)];
    assert r[b + |mid| + |RegenClose|..] == tail;
    ReadBackClose(r, sp.open, b, mid, tail);
    var k := LineEnd(mid, 0);
    LineEndFacts(mid, 0);
    assert r[..b] == s[..b];
    forall i | 0 <= i < b + k ensures r[i] != '\n' {
      if i < b { assert r[i] == r[..b][i]; } else { assert r[i] == mid[i - b]; }
    }
    assert r[b + k] == mid[k];
    LineEndIs(r, 0, b + k);
    assert r[..b + k] == s[..b] + mid[..k];
    ReadLineIgnoresTrailingSpace(s[..b], mid[..k], m);
    var o := sp.open + |RegenTag|;
    assert r[o..b + k] == s[o..b] + mid[..k];
    TrimRightSpaceTail(s[o..b], mid[..k]);
    assert s[o..b][b - o - 1] == s[sp.arrow..b][2];
    TrimRightStops(s[o..b]);
    assert s[o..b][b - o - |RegenArrow|..] == s[sp.arrow..b];
  }

  // A block whose arrow is on a later line than its open tag, given a body that starts a
  // new line: block form, found on the same lines.
  lemma ReadBackMulti(s: string, m: bool, sp: Span, mid: string, tail: string)
    requires WellPlaced(s, sp)
    requires var e := LineEnd(s, 0);
      && ReadLine(s[..e], m) == OpensBlock(sp.open)
      && e < |s|
      && !HasSuffix(TrimRight(s[sp.open + |RegenTag|..e]), RegenArrow)
      && FindArrowLine(s, e + 1) == Some(sp.body)
    requires SubstringAt(s, RegenClose, sp.close)
    requires !NoNewline(mid) && FirstLineSpace(mid) && LastLineSpace(mid) && FirstLineSpace(tail)
    requires NoCloseIn(Put(s, sp, mid, tail), sp.body, sp.body + |mid|)
    ensures NextBlock(Put(s, sp, mid, tail), m)
         == Some(Span(sp.open, sp.arrow, sp.body, sp.body + |mid|))
  {
    var e := LineEnd(s, 0);
    LineEndFacts(s, 0);
    var r := Put(s, sp, mid, tail);
    var b := sp.body;
    assert r[b..b + |mid|] == mid;
    assert r[b + |mid|..b + |mid| + |RegenClose|] == s[sp.close..End(sp)];
    assert r[b + |mid| + |RegenClose|..] == tail;
    ReadBackClose(r, sp.open, b, mid, tail);
    assert r[..b] == s[..b];
    assert r[..e + 1] == s[..e + 1] by { assert r[..e + 1] == r[..b][..e + 1]; }
    forall i | 0 <= i < e ensures r[i] != '\n' { assert r[i] == r[..e + 1][i]; }
    assert r[e] == r[..e + 1][e];
    LineEndIs(r, 0, e);
    assert r[..e] == s[..e] by { assert r[..e] == r[..e + 1][..e]; }
    assert r[sp.open + |RegenTag|..e] == s[sp.open + |RegenTag|..e]
      by { assert r[sp.open + |RegenTag|..e] == r[..e][sp.open + |RegenTag|..]; }
    ArrowLineShared(s, r, e + 1, b);
  }

  // An inline block that stands alone, read as far as its arrow, is an open tag that starts
  // its line.
  lemma OpensToTheArrow(s: string, m: bool, sp: Span)
    requires WellPlaced(s, sp)
    requires ReadLine(s[..LineEnd(s, 0)], m) == InlineBlock(sp)
    requires !m && AllSpace(s[..sp.open])
    ensures ReadLine(s[..sp.body], m) == OpensBlock(sp.open)
    ensures forall i :: 0 <= i < sp.body ==> s[i] != '\n'
  {
    var e := LineEnd(s, 0);
    LineEndFacts(s, 0);
    var x := s[..e];
    var y := s[..sp.body];
    assert y == x[..sp.body];
    assert x[..sp.body] == y[..sp.body];
    FindAgreesOnSharedPrefix(x, y, RegenTag, 0, sp.body);
    FindAgreesOnSharedPrefix(x, y, RegenArrow, sp.open + |RegenTag|, sp.body);
    assert FindFrom(y, RegenClose, sp.body) == None;
    assert y[..sp.open] == s[..sp.open];
  }

  // The first line held no block: the step is on the next line, which starts one.
  lemma {:isolate_assertions} FitsAfterALine(s: string, m: bool, e: nat, sp: Span, mid: string, tail: string)
    requires e < |s| && s[e] == '\n'
    requires WellPlaced(s[e + 1..], sp)
    requires Fits(s, m, Shift(sp, e + 1), mid, tail)
    ensures Fits(s[e + 1..], false, sp, mid, tail)
  {
    var d := e + 1;
    var t := s[d..];
    ShiftedFacts(s, e, sp);
    var o := sp.open + d;
    var ls := LineStart(s, o);
    LineStartFacts(s, o);
    assert ls >= d;
    LineStartOfSlice(s, d, |s|, sp.open);
    assert t == s[d..|s|];
    assert LineStart(t, sp.open) == ls - d;
    assert t[ls - d..sp.open] == s[ls..o];
    assert OpensALine(t, false, sp.open) == OpensALine(s, m, o);
    assert InlineAt(t, sp) == InlineAt(s, Shift(sp, d));
    hide *;
    reveal Fits, Shift, WellPlaced, End;
  }

  // `NextBlock` past a line with no block on it.
  lemma NextBlockPastALine(s: string, m: bool)
    requires LineEnd(s, 0) < |s|
    requires ReadLine(s[..LineEnd(s, 0)], m) == NoBlock
    ensures var d := LineEnd(s, 0) + 1;
      NextBlock(s, m) == match NextBlock(s[d..], false)
        case None => None
        case Some(sp) => Some(Shift(sp, d))
  {}

  lemma PutAfterALine(s: string, d: nat, sp: Span, mid: string, tail: string)
    requires d <= |s| && WellPlaced(s[d..], sp)
    ensures WellPlaced(s, Shift(sp, d))
    ensures Put(s, Shift(sp, d), mid, tail) == s[..d] + Put(s[d..], sp, mid, tail)
  {
    var t := s[d..];
    assert t[..sp.body] == s[d..sp.body + d];
    assert t[sp.close..End(sp)] == s[sp.close + d..End(sp) + d];
    assert s[..sp.body + d] == s[..d] + s[d..sp.body + d];
  }

  lemma {:isolate_assertions} ReadBackNextLine(s: string, m: bool, sp: Span, mid: string, tail: string)
    requires NextBlock(s, m) == Some(sp)
    requires ReadLine(s[..LineEnd(s, 0)], m) == NoBlock
    requires Fits(s, m, sp, mid, tail)
    requires NoCloseIn(Put(s, sp, mid, tail), sp.body, sp.body + |mid|)
    ensures NextBlock(Put(s, sp, mid, tail), m)
         == Some(Span(sp.open, sp.arrow, sp.body, sp.body + |mid|))
    decreases |s|, 0
  {
    var e := LineEnd(s, 0);
    LineEndFacts(s, 0);
    var d := e + 1;
    NextBlockPastALine(s, m);
    var t := s[d..];
    var sp0 := NextBlock(t, false).value;
    assert sp == Shift(sp0, d);
    FitsAfterALine(s, m, e, sp0, mid, tail);
    var r := Put(s, sp, mid, tail);
    PutAfterALine(s, d, sp0, mid, tail);
    var r0 := Put(t, sp0, mid, tail);
    assert r == s[..d] + r0;
    hide Put, NextBlock, Fits;
    forall j | sp0.body <= j < sp0.body + |mid| ensures !SubstringAt(r0, RegenClose, j) {
      if SubstringAt(r0, RegenClose, j) {
        assert r0[j..j + |RegenClose|] == r[j + d..j + d + |RegenClose|];
        assert SubstringAt(r, RegenClose, j + d);
      }
    }
    ReadBack(t, false, sp0, mid, tail);
    assert r[..e] == s[..e];
    forall i | 0 <= i < e ensures r[i] != '\n' { assert r[i] == s[i]; }
    assert r[e] == s[e];
    LineEndIs(r, 0, e);
    assert r[d..] == r0;
    NextBlockPastALine(r, m);
  }

  lemma ReadBack(s: string, m: bool, sp: Span, mid: string, tail: string)
    requires NextBlock(s, m) == Some(sp)
    requires Fits(s, m, sp, mid, tail)
    requires NoCloseIn(Put(s, sp, mid, tail), sp.body, sp.body + |mid|)
    ensures NextBlock(Put(s, sp, mid, tail), m)
         == Some(Span(sp.open, sp.arrow, sp.body, sp.body + |mid|))
    decreases |s|, 1
  {
    NextBlockFacts(s, m);
    var e := LineEnd(s, 0);
    LineEndFacts(s, 0);
    match ReadLine(s[..e], m)
      case InlineBlock(_) =>
        if NoNewline(mid) {
          ReadBackInline(s, m, sp, mid, tail);
        } else {
          assert InlineAt(s, sp) by {
            forall i | 0 <= i < sp.close - sp.body ensures s[sp.body..sp.close][i] != '\n' {
              assert s[sp.body..sp.close][i] == s[sp.body + i];
            }
          }
          LineStartIs(s, sp.open, 0);
          OpensToTheArrow(s, m, sp);
          assert SubstringAt(s, RegenArrow, sp.arrow) by {
            assert s[..e][sp.arrow..sp.body] == s[sp.arrow..sp.body];
          }
          ReadBackBlockSameLine(s, m, sp, mid, tail);
        }
      case OpensBlock(o) =>
        BlockFormFacts(s, o, e);
        var t := TrimRight(s[o + |RegenTag|..e]);
        if HasSuffix(t, RegenArrow) {
          TrimRightIsPrefix(s[o + |RegenTag|..e]);
          var b := sp.body;
          assert b == o + |RegenTag| + |t|;
          assert s[..e] == s[..b] + s[b..e];
          assert s[b..e] == s[o + |RegenTag|..e][|t|..];
          ReadLineIgnoresTrailingSpace(s[..b], s[b..e], m);
          assert s[sp.arrow..b] == t[|t| - |RegenArrow|..] by {
            assert t == s[o + |RegenTag|..e][..|t|];
          }
          ReadBackBlockSameLine(s, m, sp, mid, tail);
        } else {
          ReadBackMulti(s, m, sp, mid, tail);
        }
      case NoBlock =>
        ReadBackNextLine(s, m, sp, mid, tail);
  }

  // ── A block the rewrite leaves alone ──────────────────────────────────────────
  //
  // A block whose body is not rewritten is read again from the text before its close tag
  // and the first line after it. A block-form body may hold a close tag in the middle of a
  // line — the scan only stops at one that stands alone — so this is not `ReadBack` with the
  // old body put back: that asks for no close tag in the body at all.

  // Where `FindArrowLine` lands: the rest of that line is whitespace.
  lemma FindArrowLineFacts(s: string, from: nat)
    requires from <= |s|
    requires FindArrowLine(s, from).Some?
    ensures var b := FindArrowLine(s, from).value;
      AllSpace(s[b..LineEnd(s, b)])
    decreases |s| - from
  {
    var e := LineEnd(s, from);
    LineEndFacts(s, from);
    var line := s[from..e];
    var t := TrimRight(line);
    if HasSuffix(t, RegenArrow) {
      TrimRightIsPrefix(line);
      var b := from + |t|;
      LineEndIs(s, b, e);
      assert s[b..e] == line[|t|..];
    } else {
      FindArrowLineFacts(s, e + 1);
    }
  }

  // Two texts that agree up to a character that is neither a space nor a newline agree on
  // whether their first line is blank.
  lemma FirstLineShared(x: string, y: string, p: nat)
    requires 0 < p <= |x| && p <= |y| && x[..p] == y[..p]
    requires !IsSpace(x[p - 1])
    ensures FirstLineSpace(x) == FirstLineSpace(y)
  {
    var ex := LineEnd(x, 0);
    LineEndFacts(x, 0);
    forall k | 0 <= k < p ensures x[k] == y[k] { assert x[k] == x[..p][k]; assert y[k] == y[..p][k]; }
    if ex < p {
      forall k | 0 <= k < ex ensures y[k] != '\n' { assert x[k] != '\n'; }
      LineEndIs(y, 0, ex);
      assert x[..ex] == y[..ex];
    } else {
      assert x[..ex][p - 1] == x[p - 1];
      forall k | 0 <= k < p ensures y[k] != '\n' { assert x[k] != '\n'; }
      LineEndAtLeast(y, 0, p);
      assert y[..LineEnd(y, 0)][p - 1] == y[p - 1];
    }
  }

  // The same for whether a position opens its line.
  lemma OpensALineShared(s: string, r: string, m: bool, o: nat)
    requires o <= |s| && o <= |r| && r[..o] == s[..o]
    ensures OpensALine(r, m, o) == OpensALine(s, m, o)
  {
    var l := LineStart(s, o);
    LineStartFacts(s, o);
    forall k | 0 <= k < o ensures r[k] == s[k] { assert r[k] == r[..o][k]; assert s[k] == s[..o][k]; }
    LineStartIs(r, o, l);
    assert r[l..o] == s[l..o];
  }

  // A rewrite starts where the text did: its first block's open tag, if any, is still there,
  // so the first line is as blank as it was.
  lemma RewriteKeepsFirstLine(x: string, m: bool, command: string, nc: string)
    ensures FirstLineSpace(Rewrite(x, m, command, nc)) == FirstLineSpace(x)
  {
    if NextBlock(x, m).Some? {
      var sp := NextBlock(x, m).value;
      NextBlockFacts(x, m);
      TagShapes();
      var r := Rewrite(x, m, command, nc);
      var p := sp.open + 1;
      assert r[..p] == x[..p];
      assert x[sp.open] == x[sp.open..sp.open + |RegenTag|][0];
      FirstLineShared(x, r, p);
    }
  }

  // Two texts that agree through a block's close tag, and — when the block is block form —
  // on the close tag standing alone, read the same first block. One lemma per way the first
  // line can read: proved together, the three branches share one query and it runs close to
  // the time limit.
  lemma NextBlockPrefix(s: string, r: string, m: bool, sp: Span)
    requires NextBlock(s, m) == Some(sp)
    requires End(sp) <= |r| && r[..End(sp)] == s[..End(sp)]
    requires !InlineAt(s, sp) ==> FirstLineSpace(r[End(sp)..])
    ensures NextBlock(r, m) == Some(sp)
    decreases |s|, 1
  {
    var e := LineEnd(s, 0);
    match ReadLine(s[..e], m)
      case InlineBlock(_) => NextBlockPrefixInline(s, r, m, sp);
      case OpensBlock(o) => NextBlockPrefixOpens(s, r, m, sp, o);
      case NoBlock => NextBlockPrefixPastALine(s, r, m, sp);
  }

  // Both texts agree on every character before the block's end.
  lemma SharedBeforeEnd(s: string, r: string, sp: Span)
    requires End(sp) <= |s| && End(sp) <= |r| && r[..End(sp)] == s[..End(sp)]
    ensures forall k :: 0 <= k < End(sp) ==> r[k] == s[k]
  {
    forall k | 0 <= k < End(sp) ensures r[k] == s[k] {
      assert r[k] == r[..End(sp)][k]; assert s[k] == s[..End(sp)][k];
    }
  }

  lemma {:isolate_assertions} NextBlockPrefixInline(s: string, r: string, m: bool, sp: Span)
    requires NextBlock(s, m) == Some(sp)
    requires End(sp) <= |r| && r[..End(sp)] == s[..End(sp)]
    requires ReadLine(s[..LineEnd(s, 0)], m).InlineBlock?
    ensures NextBlock(r, m) == Some(sp)
    decreases |s|, 0
  {
    TagShapes();
    var e := LineEnd(s, 0);
    LineEndFacts(s, 0);
    SharedBeforeEnd(s, r, sp);
    var x := s[..e];
    assert ReadLine(x, m) == InlineBlock(sp);
    forall k | 0 <= k < End(sp) ensures r[k] != '\n' { assert s[k] == x[k]; }
    LineEndAtLeast(r, 0, End(sp));
    var x' := r[..LineEnd(r, 0)];
    assert x[..End(sp)] == x'[..End(sp)];
    FindAgreesOnSharedPrefix(x, x', RegenTag, 0, End(sp));
    FindAgreesOnSharedPrefix(x, x', RegenArrow, sp.open + |RegenTag|, End(sp));
    FindAgreesOnSharedPrefix(x, x', RegenClose, sp.body, End(sp));
    assert ReadLine(x', m) == InlineBlock(sp);
  }

  lemma {:isolate_assertions} NextBlockPrefixOpens(s: string, r: string, m: bool, sp: Span, o: nat)
    requires NextBlock(s, m) == Some(sp)
    requires End(sp) <= |r| && r[..End(sp)] == s[..End(sp)]
    requires ReadLine(s[..LineEnd(s, 0)], m) == OpensBlock(o)
    requires !InlineAt(s, sp) ==> FirstLineSpace(r[End(sp)..])
    ensures NextBlock(r, m) == Some(sp)
    decreases |s|, 0
  {
    TagShapes();
    var e := LineEnd(s, 0);
    LineEndFacts(s, 0);
    SharedBeforeEnd(s, r, sp);
    BlockFormFacts(s, o, e);
    var t := TrimRight(s[o + |RegenTag|..e]);
    var c := sp.close;
    FirstLineOfSuffix(r, End(sp));
    if HasSuffix(t, RegenArrow) {
      assert e < c;
      forall k | 0 <= k < e ensures r[k] != '\n' {}
      LineEndIs(r, 0, e);
      assert r[..e] == s[..e];
      assert r[o + |RegenTag|..e] == s[o + |RegenTag|..e];
      CloseLineShared(s, r, e + 1, c);
    } else {
      var b := FindArrowLine(s, e + 1).value;
      var le := LineEnd(s, b);
      LineEndFacts(s, b);
      FindArrowLineFacts(s, e + 1);
      assert le < c;
      forall k | 0 <= k < e ensures r[k] != '\n' {}
      LineEndIs(r, 0, e);
      assert r[..e] == s[..e];
      assert r[o + |RegenTag|..e] == s[o + |RegenTag|..e];
      forall k | b <= k < le ensures r[k] != '\n' {}
      LineEndIs(r, b, le);
      assert r[b..le] == s[b..le];
      assert r[..b] == s[..b];
      ArrowLineShared(s, r, e + 1, b);
      CloseLineShared(s, r, le + 1, c);
    }
  }

  lemma {:isolate_assertions} NextBlockPrefixPastALine(s: string, r: string, m: bool, sp: Span)
    requires NextBlock(s, m) == Some(sp)
    requires End(sp) <= |r| && r[..End(sp)] == s[..End(sp)]
    requires ReadLine(s[..LineEnd(s, 0)], m) == NoBlock
    requires !InlineAt(s, sp) ==> FirstLineSpace(r[End(sp)..])
    ensures NextBlock(r, m) == Some(sp)
    decreases |s|, 0
  {
    TagShapes();
    var e := LineEnd(s, 0);
    LineEndFacts(s, 0);
    SharedBeforeEnd(s, r, sp);
    var d := e + 1;
    var sp0 := NextBlock(s[d..], false).value;
    ShiftedFacts(s, e, sp0);
    forall k | 0 <= k < e ensures r[k] != '\n' {}
    LineEndIs(r, 0, e);
    assert r[..e] == s[..e];
    NextBlockPastALine(r, m);
    assert r[d..][..End(sp0)] == s[d..][..End(sp0)];
    assert r[d..][End(sp0)..] == r[End(sp)..];
    NextBlockPrefix(s[d..], r[d..], false, sp0);
  }

  // ── One step of the rewrite ───────────────────────────────────────────────────

  // The body a step writes holds no close tag. That is `CloseTagNotInBody`'s business; a
  // body the step leaves alone goes through `NextBlockPrefix` instead.
  lemma MidHoldsNoClose(s: string, m: bool, sp: Span, command: string, nc: string, tail: string)
    requires NextBlock(s, m) == Some(sp)
    requires Writes(s, m, sp, command, nc)
    requires ContainsNo(nc, RegenClose)
    ensures var mid := Mid(s, m, sp, command, nc);
      NoCloseIn(Put(s, sp, mid, tail), sp.body, sp.body + |mid|)
  {
    var mid := Mid(s, m, sp, command, nc);
    var r := Put(s, sp, mid, tail);
    assert r[sp.body..sp.body + |mid|] == mid;
    assert SubstringAt(r, RegenClose, sp.body + |mid|) by {
      NextBlockFacts(s, m);
      assert r[sp.body + |mid|..sp.body + |mid| + |RegenClose|] == s[sp.close..End(sp)];
    }
    CloseTagNotInBody(r, nc, sp.body, InlineAt(s, sp));
  }

  // A body `update_regen` writes fits the lines around it. A multi-line body starts and ends
  // with a newline; in a block that was inline it goes in only where the block stood alone,
  // and the rewrite keeps the line after the block as blank as it was.
  lemma MidFits(s: string, m: bool, sp: Span, command: string, nc: string)
    requires NextBlock(s, m) == Some(sp)
    requires Writes(s, m, sp, command, nc)
    ensures Fits(s, m, sp, Mid(s, m, sp, command, nc), Rewrite(s[End(sp)..], true, command, nc))
  {
    NextBlockFacts(s, m);
    RewriteKeepsFirstLine(s[End(sp)..], true, command, nc);
    var mid := RegenBody(nc, InlineAt(s, sp));
    if NoNewline(mid) {
      if !(InlineAt(s, sp) && NoNewline(nc)) { assert mid[0] == '\n'; }
    } else {
      assert mid[0] == '\n' && mid[|mid| - 1] == '\n';
      LineEndIs(mid, 0, 0);
      LineStartIs(mid, |mid|, |mid|);
      if InlineAt(s, sp) { assert !NoNewline(nc); }
    }
  }

  // One step of `Rewrite`, named, and read back.
  lemma Step(s: string, m: bool, command: string, nc: string) returns (sp: Span, sp': Span)
    requires NextBlock(s, m).Some?
    requires ContainsNo(nc, RegenClose)
    ensures sp == NextBlock(s, m).value
    ensures
      var mid  := Mid(s, m, sp, command, nc);
      var tail := Rewrite(s[End(sp)..], true, command, nc);
      var r    := Rewrite(s, m, command, nc);
      && sp' == Span(sp.open, sp.arrow, sp.body, sp.body + |mid|)
      && r == s[..sp.body] + mid + s[sp.close..End(sp)] + tail
      && NextBlock(r, m) == Some(sp')
      && CommandAt(r, sp') == CommandAt(s, sp)
      && InlineAt(r, sp') == NoNewline(mid)
      && StandsAlone(r, m, sp') == StandsAlone(s, m, sp)
      && r[..sp.body] == s[..sp.body]
      && r[sp.body..sp.body + |mid|] == mid
      && r[sp'.close..End(sp')] == s[sp.close..End(sp)]
      && r[End(sp')..] == tail
  {
    sp := NextBlock(s, m).value;
    var mid := Mid(s, m, sp, command, nc);
    sp' := Span(sp.open, sp.arrow, sp.body, sp.body + |mid|);
    var tail := Rewrite(s[End(sp)..], true, command, nc);
    var r := Rewrite(s, m, command, nc);
    assert r == Put(s, sp, mid, tail);
    NextBlockFacts(s, m);
    RewriteKeepsFirstLine(s[End(sp)..], true, command, nc);
    if Writes(s, m, sp, command, nc) {
      MidHoldsNoClose(s, m, sp, command, nc, tail);
      MidFits(s, m, sp, command, nc);
      ReadBack(s, m, sp, mid, tail);
    } else {
      assert s[..sp.body] + s[sp.body..sp.close] + s[sp.close..End(sp)] == s[..End(sp)];
      assert r[..End(sp)] == s[..End(sp)];
      assert r[End(sp)..] == tail;
      NextBlockPrefix(s, r, m, sp);
    }
    assert r[..sp.arrow] == s[..sp.arrow];
    assert r[..sp.body] == s[..sp.body];
    OpensALineShared(s, r, m, sp.open);
    assert r[sp.body..sp.body + |mid|] == mid;
    assert r[End(sp')..] == tail;
  }

  // ── What the rewrite does, block by block ─────────────────────────────────────

  // Every block `update_regen` writes holds what it would write. Stated block by block down
  // the scan, so it says *every* and not *the first*: a model that rewrote only the first
  // match would leave a second one holding the old value, and fail this.
  predicate EveryWritten(s: string, m: bool, command: string, nc: string)
    decreases |s|
  {
    match NextBlock(s, m)
      case None => true
      case Some(sp) =>
        && (Writes(s, m, sp, command, nc) ==> s[sp.body..sp.close] == RegenBody(nc, InlineAt(s, sp)))
        && EveryWritten(s[End(sp)..], true, command, nc)
  }

  // A text whose every such block already holds the content is one `Rewrite` leaves alone.
  lemma RewriteFixesTheWritten(s: string, m: bool, command: string, nc: string)
    requires EveryWritten(s, m, command, nc)
    ensures Rewrite(s, m, command, nc) == s
    decreases |s|
  {
    if NextBlock(s, m).Some? {
      var sp := NextBlock(s, m).value;
      RewriteFixesTheWritten(s[End(sp)..], true, command, nc);
      assert Mid(s, m, sp, command, nc) == s[sp.body..sp.close];
      assert s[..sp.body] + s[sp.body..sp.close] + s[sp.close..End(sp)] + s[End(sp)..] == s;
    }
  }

  // …and `Rewrite` produces one. A block the step wrote reads back in the form its new body
  // has, and that form writes the same body (`RegenBodyKeepsItsForm`). A block it left alone
  // is read at the same place with the same lines around it, so it still cannot hold the
  // content and is left alone again.
  lemma RewriteWritesEvery(s: string, m: bool, command: string, nc: string)
    requires ContainsNo(nc, RegenClose)
    ensures EveryWritten(Rewrite(s, m, command, nc), m, command, nc)
    decreases |s|
  {
    if NextBlock(s, m).Some? {
      var sp, sp' := Step(s, m, command, nc);
      var r := Rewrite(s, m, command, nc);
      RewriteWritesEvery(s[End(sp)..], true, command, nc);
      if Writes(s, m, sp, command, nc) {
        var mid := RegenBody(nc, InlineAt(s, sp));
        RegenBodyKeepsItsForm(nc, InlineAt(s, sp));
        if NoNewline(mid) && !(InlineAt(s, sp) && NoNewline(nc)) { assert mid[0] == '\n'; }
      }
    }
  }

  // The scan of the result reads the blocks the text had, with the same commands, in the
  // same order. The bases are free because the offsets are not the claim: every one after
  // the first rewritten body has moved.
  lemma RewriteKeepsTheScan(s: string, m: bool, command: string, nc: string, b1: nat, b2: nat)
    requires ContainsNo(nc, RegenClose)
    ensures Commands(ScanAt(Rewrite(s, m, command, nc), m, b1)) == Commands(ScanAt(s, m, b2))
    decreases |s|
  {
    if NextBlock(s, m).Some? {
      var sp, sp' := Step(s, m, command, nc);
      var r := Rewrite(s, m, command, nc);
      var rest := s[End(sp)..];
      RewriteKeepsTheScan(rest, true, command, nc, b1 + End(sp'), b2 + End(sp));
      var rb := Block(CommandAt(r, sp'), Shift(sp', b1), InlineAt(r, sp'));
      var sb := Block(CommandAt(s, sp), Shift(sp, b2), InlineAt(s, sp));
      ScanAtSteps(r, m, b1, sp');
      ScanAtSteps(s, m, b2, sp);
      CommandsOfCons(rb, ScanAt(Rewrite(rest, true, command, nc), true, b1 + End(sp')));
      CommandsOfCons(sb, ScanAt(rest, true, b2 + End(sp)));
    } else {
      assert Rewrite(s, m, command, nc) == s;
    }
  }

  // The text with the body of every block named `command` taken out. Two texts that agree
  // here agree on every byte `update_regen` could be asked to write.
  function Hollow(s: string, m: bool, command: string): string
    decreases |s|
  {
    match NextBlock(s, m)
      case None => s
      case Some(sp) =>
        s[..sp.body] + (if CommandAt(s, sp) == command then "" else s[sp.body..sp.close])
          + s[sp.close..End(sp)] + Hollow(s[End(sp)..], true, command)
  }

  lemma RewriteTouchesOnlyTheBodies(s: string, m: bool, command: string, nc: string)
    requires ContainsNo(nc, RegenClose)
    ensures Hollow(Rewrite(s, m, command, nc), m, command) == Hollow(s, m, command)
    decreases |s|
  {
    if NextBlock(s, m).Some? {
      var sp, sp' := Step(s, m, command, nc);
      RewriteTouchesOnlyTheBodies(s[End(sp)..], true, command, nc);
    }
  }

  // A block after the first is read with `m`, and whether it stands alone does not depend
  // on that: the line it would share is the one the close tag before it ends with `>` on.
  lemma StandsAloneAfterABlock(s: string, m: bool, sp: Span, q: Span)
    requires NextBlock(s, m) == Some(sp)
    requires WellPlaced(s[End(sp)..], q)
    ensures WellPlaced(s, Shift(q, End(sp)))
    ensures StandsAlone(s, m, Shift(q, End(sp))) == StandsAlone(s[End(sp)..], true, q)
  {
    NextBlockFacts(s, m);
    TagShapes();
    var e := End(sp);
    var t := s[e..];
    assert s[e - 1] == '>' by { assert s[sp.close..e][14] == RegenClose[14]; }
    var o := q.open + e;
    assert t == s[e..|s|];
    LineStartOfSlice(s, e, |s|, q.open);
    LineStartFacts(s, o);
    var l := LineStart(s, o);
    if l > e {
      assert t[l - e..q.open] == s[l..o];
    } else if l < e {
      assert s[l..o][e - 1 - l] == s[e - 1];
    }
    assert s[End(Shift(q, e))..] == t[End(q)..];
  }

  lemma WrittenAfterABlock(s: string, m: bool, sp: Span, q: Span, nc: string)
    requires NextBlock(s, m) == Some(sp)
    requires WellPlaced(s[End(sp)..], q)
    ensures WellPlaced(s, Shift(q, End(sp)))
    ensures InlineAt(s, Shift(q, End(sp))) == InlineAt(s[End(sp)..], q)
    ensures Written(s, m, Shift(q, End(sp)), nc) == Written(s[End(sp)..], true, q, nc)
  {
    StandsAloneAfterABlock(s, m, sp, q);
    SuffixSlice(s, End(sp), q.body, q.close);
  }

  // The first block named `command`, before and after. Everything up to its body is
  // untouched, its body is what the rewrite wrote there — the new one if it can hold it, the
  // old one if not — its close tag follows directly, and the scan of the result selects it at
  // the same open tag and arrow. `q` is the block's span relative to `s` — returned rather
  // than computed in the postcondition, where subtracting `base` from a `nat` would need this
  // lemma's conclusion to be well-formed.
  lemma RewriteAtTheFirstMatch(s: string, m: bool, command: string, nc: string, base: nat)
    returns (q: Span)
    requires ContainsNo(nc, RegenClose)
    requires FirstWithCommand(ScanAt(s, m, base), command).Some?
    ensures FirstWithCommand(ScanAt(s, m, base), command) == Some(Shift(q, base))
    ensures WellPlaced(s, q)
    ensures
      var r    := Rewrite(s, m, command, nc);
      var body := Written(s, m, q, nc);
      && q.body + |body| + |RegenClose| <= |r|
      && r[..q.body] == s[..q.body]
      && r[q.body..q.body + |body|] == body
      && SubstringAt(r, RegenClose, q.body + |body|)
      && FirstWithCommand(ScanAt(r, m, base), command)
         == Some(Shift(Span(q.open, q.arrow, q.body, q.body + |body|), base))
    decreases |s|, 1
  {
    ScanAtSteps(s, m, base, NextBlock(s, m).value);
    var sp := NextBlock(s, m).value;
    FirstWithCommandOfCons(Block(CommandAt(s, sp), Shift(sp, base), InlineAt(s, sp)),
                           ScanAt(s[End(sp)..], true, base + End(sp)), command);
    if CommandAt(s, sp) == command {
      q := FirstMatchIsHere(s, m, command, nc, base);
    } else {
      q := FirstMatchIsLater(s, m, command, nc, base);
    }
  }

  // The first block is the one: `Step` has already said everything.
  lemma FirstMatchIsHere(s: string, m: bool, command: string, nc: string, base: nat)
    returns (q: Span)
    requires NextBlock(s, m).Some? && CommandAt(s, NextBlock(s, m).value) == command
    requires ContainsNo(nc, RegenClose)
    requires FirstWithCommand(ScanAt(s, m, base), command).Some?
    ensures FirstWithCommand(ScanAt(s, m, base), command) == Some(Shift(q, base))
    ensures WellPlaced(s, q)
    ensures
      var r    := Rewrite(s, m, command, nc);
      var body := Written(s, m, q, nc);
      && q.body + |body| + |RegenClose| <= |r|
      && r[..q.body] == s[..q.body]
      && r[q.body..q.body + |body|] == body
      && SubstringAt(r, RegenClose, q.body + |body|)
      && FirstWithCommand(ScanAt(r, m, base), command)
         == Some(Shift(Span(q.open, q.arrow, q.body, q.body + |body|), base))
  {
    var sp, sp' := Step(s, m, command, nc);
    var r := Rewrite(s, m, command, nc);
    q := sp;
    assert Mid(s, m, sp, command, nc) == Written(s, m, sp, nc);
    ScanAtSteps(s, m, base, sp);
    ScanAtSteps(r, m, base, sp');
    FirstWithCommandOfCons(Block(CommandAt(s, sp), Shift(sp, base), InlineAt(s, sp)),
                           ScanAt(s[End(sp)..], true, base + End(sp)), command);
    FirstWithCommandOfCons(Block(CommandAt(r, sp'), Shift(sp', base), InlineAt(r, sp')),
                           ScanAt(r[End(sp')..], true, base + End(sp')), command);
    NextBlockFacts(s, m);
  }

  // The first block is not the one, so the step left it as it was and the answer is in the
  // rest — at the same place, moved by the length of the block in front of it.
  lemma FirstMatchIsLater(s: string, m: bool, command: string, nc: string, base: nat)
    returns (q: Span)
    requires NextBlock(s, m).Some? && CommandAt(s, NextBlock(s, m).value) != command
    requires ContainsNo(nc, RegenClose)
    requires FirstWithCommand(ScanAt(s, m, base), command).Some?
    ensures FirstWithCommand(ScanAt(s, m, base), command) == Some(Shift(q, base))
    ensures WellPlaced(s, q)
    ensures
      var r    := Rewrite(s, m, command, nc);
      var body := Written(s, m, q, nc);
      && q.body + |body| + |RegenClose| <= |r|
      && r[..q.body] == s[..q.body]
      && r[q.body..q.body + |body|] == body
      && SubstringAt(r, RegenClose, q.body + |body|)
      && FirstWithCommand(ScanAt(r, m, base), command)
         == Some(Shift(Span(q.open, q.arrow, q.body, q.body + |body|), base))
    decreases |s|, 0
  {
    hide Rewrite, ScanAt;
    var sp := NextBlock(s, m).value;
    var e := End(sp);
    var rest := s[e..];
    var tail := Rewrite(rest, true, command, nc);
    var r := Rewrite(s, m, command, nc);
    RewriteSkips(s, m, command, nc);
    var sb := Block(CommandAt(s, sp), Shift(sp, base), InlineAt(s, sp));
    var rb := Block(CommandAt(r, sp), Shift(sp, base), InlineAt(r, sp));
    ScanAtSteps(s, m, base, sp);
    ScanAtSteps(r, m, base, sp);
    FirstWithCommandOfCons(sb, ScanAt(rest, true, base + e), command);
    FirstWithCommandOfCons(rb, ScanAt(tail, true, base + e), command);
    assert FirstWithCommand(ScanAt(rest, true, base + e), command)
        == FirstWithCommand(ScanAt(s, m, base), command);
    assert |rest| < |s|;
    assert FirstWithCommand(ScanAt(rest, true, base + e), command).Some?;
    var q' := RewriteAtTheFirstMatch(rest, true, command, nc, base + e);
    q := Shift(q', e);
    assert Shift(q, base) == Shift(q', base + e);
    WrittenAfterABlock(s, m, sp, q', nc);
    var body := Written(s, m, q, nc);
    assert Shift(Span(q.open, q.arrow, q.body, q.body + |body|), base)
        == Shift(Span(q'.open, q'.arrow, q'.body, q'.body + |body|), base + e);
    PrefixedSlices(s[..e], tail, q'.body, q'.body + |body|);
    PrefixedSlices(s[..e], tail, q'.body + |body|, q'.body + |body| + |RegenClose|);
    PrefixedSlices(s[..e], rest, q'.body, q'.body);
    assert s[..e] + rest == s;
  }

  // A step over a block not named `command` copies it whole.
  lemma RewriteSkips(s: string, m: bool, command: string, nc: string)
    requires ContainsNo(nc, RegenClose)
    requires NextBlock(s, m).Some? && CommandAt(s, NextBlock(s, m).value) != command
    ensures
      var sp := NextBlock(s, m).value;
      var r  := Rewrite(s, m, command, nc);
      && r == s[..End(sp)] + Rewrite(s[End(sp)..], true, command, nc)
      && NextBlock(r, m) == Some(sp)
      && CommandAt(r, sp) == CommandAt(s, sp)
      && InlineAt(r, sp) == InlineAt(s, sp)
      && r[End(sp)..] == Rewrite(s[End(sp)..], true, command, nc)
  {
    var sp, sp' := Step(s, m, command, nc);
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
  // The body in (2) is `Written`: the new body where the block can hold it, and the old one
  // where it cannot — a multi-line value and an inline block that shares its line (#1137).
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
      var body   := Written(text, false, sp, newContent);
      && sp.body + |body| <= |result|
      // (1) Frame: nothing before the first such block's body moves, and nothing anywhere
      //     that is not the body of a block named `command` changes.
      && result[..sp.body] == text[..sp.body]
      && Hollow(result, false, command) == Hollow(text, false, command)
      // (2) The first such block holds what was written there and is still selected, at the
      //     same open tag and arrow, with its close tag right after the body.
      && result[sp.body..sp.body + |body|] == body
      && RegenSpan(result, command) == Some(Span(sp.open, sp.arrow, sp.body, sp.body + |body|))
      && HasRegenFor(result, command)
      // (3) The scan of the result reads the blocks the text had, in the same order.
      && Commands(RegenScan(result)) == Commands(RegenScan(text))
      // (4) Every block that can hold the new content holds it — every, not the first.
      && EveryWritten(result, false, command, newContent)
      // (5) Idempotency: a second application with the same content changes nothing.
      && UpdateRegen(result, command, newContent) == result
  {
    RegenSpanFindsEveryBlock(text, command);
    var sp := RegenSpan(text, command).value;
    var q := RewriteAtTheFirstMatch(text, false, command, newContent, 0);
    assert Shift(q, 0) == q;
    var body := Written(text, false, sp, newContent);
    assert Shift(Span(sp.open, sp.arrow, sp.body, sp.body + |body|), 0)
        == Span(sp.open, sp.arrow, sp.body, sp.body + |body|);
    var result := UpdateRegen(text, command, newContent);
    RegenSpanFindsEveryBlock(result, command);
    RewriteTouchesOnlyTheBodies(text, false, command, newContent);
    RewriteKeepsTheScan(text, false, command, newContent, 0, 0);
    RewriteWritesEvery(text, false, command, newContent);
    RewriteFixesTheWritten(result, false, command, newContent);
  }

  // ── Witnesses ─────────────────────────────────────────────────────────────────
  //
  // The lemmas above are universal, and a universal statement about the empty set reads
  // exactly like one about something. Each witness below is one document with its offsets
  // worked out. Most blocks in them start at offset 0 on purpose: it keeps each `FindFrom`
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
    var q := RewriteAtTheFirstMatch(text, false, command, "", 0);
    assert Shift(q, 0) == q;
    assert Written(text, false, sp, "") == "\n";
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
    var sp := RegenSpan(text, command).value;
    assert ContainsNo("", RegenClose);
    var q := RewriteAtTheFirstMatch(text, false, command, "", 0);
    assert Shift(q, 0) == q;
    assert Written(text, false, sp, "") == "";
  }

  // The scan reads nothing from an empty rest. Every witness ends its document on a close
  // tag, and this is the step that says the scan stops there.
  lemma NothingAfterTheEnd(m: bool, base: nat)
    ensures NextBlock("", m) == None
    ensures ScanAt("", m, base) == []
    ensures forall c, nc :: Rewrite("", m, c, nc) == ""
  {
    assert LineEnd("", 0) == 0;
    assert ""[..0] == "";
    assert FindFrom("", RegenTag, 0) == None;
    assert ReadLine("", m) == NoBlock;
  }

  // A document of one line is read as one line, and the command of a block on it is what
  // lies between the tag and the arrow.
  lemma OneLine(text: string, m: bool, sp: Span)
    requires NoNewline(text)
    requires ReadLine(text, m) == InlineBlock(sp)
    ensures NextBlock(text, m) == Some(sp)
    ensures CommandAt(text, sp) == Trim(text[sp.open + |RegenTag|..sp.arrow])
  {
    LineEndIs(text, 0, |text|);
    assert text[..|text|] == text;
    var h := text[..sp.arrow];
    LineEndIs(h, sp.open + |RegenTag|, |h|);
    assert h[sp.open + |RegenTag|..|h|] == text[sp.open + |RegenTag|..sp.arrow];
  }

  // A document holding one block and ending on its close tag: the scan reads that block
  // and no other, and the rewrite touches that block's body and nothing else.
  lemma OneBlock(text: string, m: bool, sp: Span, command: string, nc: string)
    requires NextBlock(text, m) == Some(sp)
    requires End(sp) == |text|
    ensures ScanAt(text, m, 0) == [Block(CommandAt(text, sp), sp, InlineAt(text, sp))]
    ensures Rewrite(text, m, command, nc)
         == text[..sp.body] + Mid(text, m, sp, command, nc) + text[sp.close..]
  {
    NothingAfterTheEnd(true, End(sp));
    assert text[End(sp)..] == "";
    assert Shift(sp, 0) == sp;
    assert text[sp.close..End(sp)] == text[sp.close..];
  }

  lemma TrimAroundOne(c: char)
    requires !IsSpace(c)
    ensures Trim([' ', c, ' ']) == [c]
  {
    assert TrimRight([' ', c, ' ']) == [' ', c];
    assert TrimLeft([' ', c]) == [c];
  }

  // The characters and slices of the witness documents below, each proved alone. Proved in
  // place, next to the scan's definitions, the solver sometimes fails to index a string
  // literal it indexes instantly on its own.
  lemma InlineDocument()
    ensures var text := "<!-- REGEN: n -->44<!-- /REGEN -->";
            && |text| == 34 && NoNewline(text)
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
            && |text| == 34 && NoNewline(text)
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
            && |text| == 34
            && text[11] == ' ' && text[12] == 'a' && text[13] == ' '
            && text[17] == '\n' && text[18] == '\n'
            && text[0..11] == "<!-- REGEN:"
            && text[..17] == "<!-- REGEN: a -->"
            && text[11..14] == " a "
            && text[11..17] == " a -->"
            && text[14..17] == "-->"
            && text[19..34] == "<!-- /REGEN -->"
            && (forall k :: 0 <= k < 17 ==> text[k] != '\n')
            && (forall k :: 19 <= k < 34 ==> text[k] != '\n')
  {
    var text := "<!-- REGEN: a -->\n\n<!-- /REGEN -->";
    SliceIsLiteral(text, 0, 11, "<!-- REGEN:");
    SliceIsLiteral(text, 0, 17, "<!-- REGEN: a -->");
    SliceIsLiteral(text, 11, 14, " a ");
    SliceIsLiteral(text, 11, 17, " a -->");
    SliceIsLiteral(text, 14, 17, "-->");
    SliceIsLiteral(text, 19, 34, "<!-- /REGEN -->");
  }

  lemma UnanchoredDocument()
    ensures var text := "<!-- REGEN: a -->\nx<!-- /REGEN -->";
            && |text| == 34
            && text[17] == '\n' && text[18] == 'x'
            && text[..17] == "<!-- REGEN: a -->"
            && text[11..17] == " a -->"
            && text[18..34] == "x<!-- /REGEN -->"
            && (forall k :: 0 <= k < 17 ==> text[k] != '\n')
            && (forall k :: 18 <= k < 34 ==> text[k] != '\n')
  {
    var text := "<!-- REGEN: a -->\nx<!-- /REGEN -->";
    SliceIsLiteral(text, 0, 17, "<!-- REGEN: a -->");
    SliceIsLiteral(text, 11, 17, " a -->");
    SliceIsLiteral(text, 18, 34, "x<!-- /REGEN -->");
  }

  lemma MidSentenceDocument()
    ensures var first := "x<!-- REGEN: a -->";
            var rest := "<!-- REGEN: b -->1<!-- /REGEN -->";
            && |first| == 18 && NoNewline(first)
            && first[0] == 'x' && first[12] == ' ' && first[13] == 'a' && first[14] == ' '
            && first[..1] == "x"
            && first[1..12] == "<!-- REGEN:"
            && first[15..18] == "-->"
            && |rest| == 33 && NoNewline(rest)
            && rest[11] == ' ' && rest[12] == 'b' && rest[13] == ' ' && rest[17] == '1'
            && rest[0..11] == "<!-- REGEN:"
            && rest[11..14] == " b "
            && rest[14..17] == "-->"
            && rest[18..33] == "<!-- /REGEN -->"
  {
    var first := "x<!-- REGEN: a -->";
    var rest := "<!-- REGEN: b -->1<!-- /REGEN -->";
    SliceIsLiteral(first, 0, 1, "x");
    SliceIsLiteral(first, 1, 12, "<!-- REGEN:");
    SliceIsLiteral(first, 15, 18, "-->");
    SliceIsLiteral(rest, 0, 11, "<!-- REGEN:");
    SliceIsLiteral(rest, 11, 14, " b ");
    SliceIsLiteral(rest, 14, 17, "-->");
    SliceIsLiteral(rest, 18, 33, "<!-- /REGEN -->");
  }

  lemma SharedLineDocument()
    ensures var text := "x<!-- REGEN: a -->1<!-- /REGEN -->";
            && |text| == 34 && NoNewline(text)
            && text[0] == 'x' && text[12] == ' ' && text[13] == 'a' && text[14] == ' '
            && text[18] == '1'
            && text[0..1] == "x"
            && text[1..12] == "<!-- REGEN:"
            && text[12..15] == " a "
            && text[15..18] == "-->"
            && text[18..19] == "1"
            && text[19..34] == "<!-- /REGEN -->"
  {
    var text := "x<!-- REGEN: a -->1<!-- /REGEN -->";
    SliceIsLiteral(text, 0, 1, "x");
    SliceIsLiteral(text, 1, 12, "<!-- REGEN:");
    SliceIsLiteral(text, 12, 15, " a ");
    SliceIsLiteral(text, 15, 18, "-->");
    SliceIsLiteral(text, 18, 19, "1");
    SliceIsLiteral(text, 19, 34, "<!-- /REGEN -->");
  }

  // An open tag at 0 whose command is one character: the arrow is at 14.
  lemma ArrowAfterOneChar(line: string)
    requires |line| >= 17 && line[11] == ' ' && !IsSpace(line[12]) && line[13] == ' '
    requires line[14..17] == "-->"
    requires line[0..11] == "<!-- REGEN:"
    ensures FindFrom(line, RegenTag, 0) == Some(0)
    ensures FindFrom(line, RegenArrow, 11) == Some(14)
  {
    assert SubstringAt(line, RegenTag, 0);
    assert !SubstringAt(line, RegenArrow, 11) by { assert line[11..14][0] == ' '; }
    assert !SubstringAt(line, RegenArrow, 12) by { assert line[12..15][0] == line[12]; }
    assert !SubstringAt(line, RegenArrow, 13) by { assert line[13..16][0] == ' '; }
    assert SubstringAt(line, RegenArrow, 14);
  }

  // The inline block of `InlineDocument`, read.
  lemma InlineDocumentReads()
    ensures var text := "<!-- REGEN: n -->44<!-- /REGEN -->";
            && NextBlock(text, false) == Some(Span(0, 14, 17, 19))
            && CommandAt(text, Span(0, 14, 17, 19)) == "n"
            && InlineAt(text, Span(0, 14, 17, 19))
  {
    var text := "<!-- REGEN: n -->44<!-- /REGEN -->";
    InlineDocument();
    ArrowAfterOneChar(text);
    assert !SubstringAt(text, RegenClose, 17) by { assert text[17..32][0] == '4'; }
    assert !SubstringAt(text, RegenClose, 18) by { assert text[18..33][0] == '4'; }
    assert FindFrom(text, RegenClose, 17) == Some(19);
    assert ReadLine(text, false) == InlineBlock(Span(0, 14, 17, 19));
    OneLine(text, false, Span(0, 14, 17, 19));
    TrimAroundOne('n');
    assert text[17..19] == "44";
  }

  // `ClearingAnInlineSectionWritesNothing` is about something: one inline block, read and
  // rewritten.
  lemma {:isolate_assertions} AnInlineBlockIsNotAnEmptyCase()
    ensures var text := "<!-- REGEN: n -->44<!-- /REGEN -->";
            && RegenScan(text) == [Block("n", Span(0, 14, 17, 19), true)]
            && RegenSpan(text, "n") == Some(Span(0, 14, 17, 19))
            && InlineAt(text, Span(0, 14, 17, 19))
            && UpdateRegen(text, "n", "43") == "<!-- REGEN: n -->43<!-- /REGEN -->"
  {
    var text := "<!-- REGEN: n -->44<!-- /REGEN -->";
    var sp := Span(0, 14, 17, 19);
    InlineDocument();
    InlineDocumentReads();
    OneBlock(text, false, sp, "n", "43");
    hide *;
    reveal UpdateRegen, RegenScan, RegenSpan, FirstWithCommand, Mid, Writes, Holds, RegenBody;
    reveal NoNewline;
    assert NoNewline("43");
    assert RegenBody("43", true) == "43";
    assert Mid(text, false, sp, "n", "43") == "43";
    assert text[..17] == "<!-- REGEN: n -->";
    assert text[19..] == "<!-- /REGEN -->";
  }

  // A multi-line value goes into an inline block that stands alone on its line, and turns it
  // into the block form: the value on lines of its own between the two tags. The second run
  // reads that block form and writes the same body (`RegenBodyKeepsItsForm`).
  lemma {:isolate_assertions} AStandingBlockTakesMoreThanOneLine()
    ensures var text := "<!-- REGEN: n -->44<!-- /REGEN -->";
            UpdateRegen(text, "n", "4\n3") == "<!-- REGEN: n -->\n4\n3\n<!-- /REGEN -->"
  {
    var text := "<!-- REGEN: n -->44<!-- /REGEN -->";
    var sp := Span(0, 14, 17, 19);
    InlineDocument();
    InlineDocumentReads();
    OneBlock(text, false, sp, "n", "4\n3");
    assert LineStart(text, 0) == 0;
    assert text[0..0] == "";
    assert text[34..] == "";
    assert LineEnd("", 0) == 0;
    assert StandsAlone(text, false, sp);
    assert !NoNewline("4\n3") by { assert "4\n3"[1] == '\n'; }
    assert RegenBody("4\n3", true) == "\n4\n3\n";
    assert Mid(text, false, sp, "n", "4\n3") == "\n4\n3\n";
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
    assert ReadLine(text, false) == InlineBlock(sp);
    OneLine(text, false, sp);

    assert Trim(" ab ") == "ab" by {
      assert TrimRight(" ab ") == " ab";
      assert TrimLeft(" ab") == "ab";
    }
    assert CommandAt(text, sp) == "ab";
    OneBlock(text, false, sp, "a", "2");
    assert Mid(text, false, sp, "a", "2") == text[18..19];
    assert text[..18] + text[18..19] + text[19..] == text;
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
    var line := text[..17];
    TagShapes();

    // The open tag's line: the tag and the arrow, and no close tag after them.
    LineEndIs(text, 0, 17);
    ArrowAfterOneChar(line);
    assert FindFrom(line, RegenClose, 17) == None;
    assert line[..0] == "";
    assert ReadLine(line, false) == OpensBlock(0);

    // The rest of that line ends in `-->`, so the body starts after it.
    assert TrimRight(text[11..17]) == " a -->";
    // The next line is empty; the one after it is the close tag and nothing else.
    LineEndIs(text, 18, 18);
    assert text[18..18] == "";
    LineEndIs(text, 19, 34);
    assert Trim(RegenClose) == RegenClose by { PaddedClose("", ""); assert "" + RegenClose + "" == RegenClose; }
    assert |TrimRight(RegenClose)| == 15 by { PaddedClose("", ""); assert "" + RegenClose + "" == RegenClose; }
    assert FindCloseLine(text, 19) == Some(19);
    assert FindCloseLine(text, 18) == Some(19);
    assert BlockForm(text, 0, 17) == Some(sp);
    assert NextBlock(text, false) == Some(sp);

    var h := text[..14];
    LineEndIs(h, 11, 14);
    assert h[11..14] == " a ";
    TrimAroundOne('a');
    assert CommandAt(text, sp) == "a";
    assert !InlineAt(text, sp) by { assert text[17..19][0] == '\n'; }
    OneBlock(text, false, sp, "", "");
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
  // The code agrees, and says more: `scan_markers` sees an open tag inside `a`'s body, and
  // reports `a` as `ClosedOnAnothersTag` — `ABlockThatClosesOnAnothersTagIsReported` is that
  // report, in the line model.
  // The content spells an open tag and holds no close tag: '/' is at offset 5 of the close
  // tag, and the content has none.
  lemma {:isolate_assertions} ATagSpellingHoldsNoClose()
    ensures ContainsNo("<!-- REGEN: b -->", RegenClose)
  {
    var nc := "<!-- REGEN: b -->";
    assert RegenClose[5] == '/';
    forall i | 0 <= i < |nc| ensures nc[i] != '/' {}
    forall i | 0 <= i <= |nc| ensures !SubstringAt(nc, RegenClose, i) {
      if SubstringAt(nc, RegenClose, i) { assert nc[i..i + |RegenClose|][5] == nc[i + 5]; }
    }
  }

  // Written into the one block-form block, it is set on a line of its own.
  lemma {:isolate_assertions} ATagSpellingIsWritten()
    ensures var text := "<!-- REGEN: a -->\n\n<!-- /REGEN -->";
            var sp := Span(0, 14, 17, 19);
            && WellPlaced(text, sp)
            && Written(text, false, sp, "<!-- REGEN: b -->") == "\n<!-- REGEN: b -->\n"
  {
    var text := "<!-- REGEN: a -->\n\n<!-- /REGEN -->";
    var sp := Span(0, 14, 17, 19);
    BlockDocument();
    TagShapes();
    hide *;
    reveal Written, Holds, RegenBody, InlineAt, NoNewline, WellPlaced, End;
    assert WellPlaced(text, sp);
    assert !InlineAt(text, sp) by { assert text[17..19][0] == text[17]; }
  }

  // A body that starts at 17 with a newline puts what follows it at 18.
  lemma SpelledAtEighteen(result: string, body: string)
    requires body == "\n<!-- REGEN: b -->\n"
    requires 17 + |body| <= |result| && result[17..17 + |body|] == body
    ensures |result| >= 18 + |RegenOpen + "b"|
    ensures SubstringAt(result, RegenOpen + "b", 18)
  {
    assert RegenOpen + "b" == "<!-- REGEN: b";
    forall i | 0 <= i < 13 ensures result[18 + i] == "<!-- REGEN: b"[i] {
      assert result[18 + i] == result[17..17 + |body|][1 + i];
    }
    SliceIsLiteral(result, 18, 31, "<!-- REGEN: b");
  }

  lemma {:isolate_assertions} ContentThatSpellsATagIsNotABlock()
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
    var sp := Span(0, 14, 17, 19);
    TheOneBlockIsBlockForm();
    ATagSpellingHoldsNoClose();
    ATagSpellingIsWritten();
    CommandsOfCons(Block("a", sp, false), []);
    assert Commands(RegenScan(text)) == ["a"];
    assert RegenScan(text)[0] in RegenScan(text);
    assert RegenSpan(text, "a") == Some(sp);
    UpdateRegenSpec(text, "a", nc);
    var result := UpdateRegen(text, "a", nc);
    SpelledAtEighteen(result, Written(text, false, sp, nc));
    if HasRegenFor(result, "b") {
      var b :| b in RegenScan(result) && b.command == "b";
      CommandsHoldsEveryCommand(RegenScan(result), b);
    }
  }


  // ── Where the model and the code agree on a line ──────────────────────────────
  //
  // The scan reads a line at a time, as `scan_markers` does (#1137). These are the documents
  // on which a scan that ignored lines — the one this model had before — read a block the
  // code does not.

  // A close tag with text before it on its line is not the close of a block form. The code
  // reads this block as `CloseTagMissing` and records no extent, so `update_regen` leaves the
  // document alone, and so does the model. The old scan read `a` here and rewrote it.
  // `a_close_tag_that_does_not_stand_alone_is_not_a_block_form_close` in `markers.rs` is the
  // same document, run against the code.
  lemma ACloseTagMustStandAlone()
    ensures var text := "<!-- REGEN: a -->\nx<!-- /REGEN -->";
            && RegenSpan(text, "a") == None
            && UpdateRegen(text, "a", "y") == text
  {
    var text := "<!-- REGEN: a -->\nx<!-- /REGEN -->";
    UnanchoredDocument();
    var line := text[..17];
    TagShapes();
    LineEndIs(text, 0, 17);
    ArrowAfterOneChar(line);
    assert FindFrom(line, RegenClose, 17) == None;
    assert line[..0] == "";
    assert ReadLine(line, false) == OpensBlock(0);
    assert TrimRight(text[11..17]) == " a -->";
    // The only line below is the close tag with an `x` in front of it: sixteen characters,
    // which trim to sixteen, not the fifteen of the tag.
    LineEndIs(text, 18, 34);
    assert Trim(text[18..34]) != RegenClose by {
      assert text[18..34][15] == '>';
      TrimRightStops(text[18..34]);
      assert text[18..34][0] == 'x';
    }
    assert FindCloseLine(text, 18) == None;
    assert NextBlock(text, false) == None;
    assert RegenScan(text) == [];
  }

  // An open tag in the middle of a sentence, with no close tag beside it, is prose: the scan
  // goes on to the next line, and the block there is the one it reads. The old scan read a
  // block from the prose tag to `b`'s close tag and called it `a`.
  lemma {:isolate_assertions} MidSentenceNextBlock()
    ensures NextBlock("x<!-- REGEN: a -->\n<!-- REGEN: b -->1<!-- /REGEN -->", false)
         == Some(Span(19, 33, 36, 37))
  {
    var text := "x<!-- REGEN: a -->\n<!-- REGEN: b -->1<!-- /REGEN -->";
    hide NextBlock;
    var first := "x<!-- REGEN: a -->";
    var rest := "<!-- REGEN: b -->1<!-- /REGEN -->";
    assert text == first + "\n" + rest;
    MidSentenceDocument();
    TagShapes();

    // The first line: a tag at 1 and an arrow after it, no close tag, and an `x` before it.
    forall k | 0 <= k < 18 ensures text[k] != '\n' { assert text[k] == first[k]; }
    assert text[18] == '\n';
    LineEndIs(text, 0, 18);
    assert text[..18] == first;
    assert !SubstringAt(first, RegenTag, 0) by { assert first[0..11][0] == first[0]; }
    assert FindFrom(first, RegenTag, 0) == Some(1);
    assert !SubstringAt(first, RegenArrow, 12) by { assert first[12..15][0] == first[12]; }
    assert !SubstringAt(first, RegenArrow, 13) by { assert first[13..16][0] == first[13]; }
    assert !SubstringAt(first, RegenArrow, 14) by { assert first[14..17][0] == first[14]; }
    assert FindFrom(first, RegenArrow, 12) == Some(15);
    assert FindFrom(first, RegenClose, 18) == None;
    assert !AllSpace(first[..1]);
    assert ReadLine(first, false) == NoBlock;

    // The next line is a document of its own: one inline block, `b`.
    assert text[19..] == rest;
    ArrowAfterOneChar(rest);
    assert !SubstringAt(rest, RegenClose, 17) by { assert rest[17..32][0] == rest[17]; }
    assert FindFrom(rest, RegenClose, 17) == Some(18);
    var sp := Span(0, 14, 17, 18);
    assert ReadLine(rest, false) == InlineBlock(sp);
    OneLine(rest, false, sp);
    TrimAroundOne('b');
    assert CommandAt(rest, sp) == "b";
    OneBlock(rest, false, sp, "", "");
    NextBlockPastALine(text, false);
    assert NextBlock(text, false) == Some(Shift(sp, 19));
    assert Shift(sp, 19) == Span(19, 33, 36, 37);
  }

  // The command's line, up to the arrow: ` b ` and nothing after it.
  lemma MidSentenceHead()
    ensures var h := "x<!-- REGEN: a -->\n<!-- REGEN: b -->1<!-- /REGEN -->"[..33];
            && |h| == 33 && LineEnd(h, 30) == 33 && h[30..33] == " b "
  {
    var text := "x<!-- REGEN: a -->\n<!-- REGEN: b -->1<!-- /REGEN -->";
    var h := text[..33];
    assert h[30] == ' ' && h[31] == 'b' && h[32] == ' ';
    LineEndIs(h, 30, 33);
    SliceIsLiteral(h, 30, 33, " b ");
  }

  lemma {:isolate_assertions} MidSentenceCommand()
    ensures var text := "x<!-- REGEN: a -->\n<!-- REGEN: b -->1<!-- /REGEN -->";
            var sp := Span(19, 33, 36, 37);
            && NextBlock(text, false) == Some(sp)
            && WellPlaced(text, sp)
            && CommandAt(text, sp) == "b"
            && InlineAt(text, sp)
  {
    var text := "x<!-- REGEN: a -->\n<!-- REGEN: b -->1<!-- /REGEN -->";
    var sp := Span(19, 33, 36, 37);
    MidSentenceNextBlock();
    MidSentenceHead();
    TrimAroundOne('b');
    TagShapes();
    hide *;
    reveal CommandAt, WellPlaced, End, InlineAt, NoNewline;
    assert |text| == 52;
    assert WellPlaced(text, sp);
    var h := text[..33];
    assert h[30..LineEnd(h, 30)] == " b ";
    assert CommandAt(text, sp) == Trim(" b ");
    assert CommandAt(text, sp) == "b";
    assert text[36..37] == "1";
    assert InlineAt(text, sp);
  }

  lemma {:isolate_assertions} AnOpenTagMidSentenceIsProse()
    ensures var text := "x<!-- REGEN: a -->\n<!-- REGEN: b -->1<!-- /REGEN -->";
            && RegenSpan(text, "a") == None
            && RegenSpan(text, "b") == Some(Span(19, 33, 36, 37))
  {
    var text := "x<!-- REGEN: a -->\n<!-- REGEN: b -->1<!-- /REGEN -->";
    var sp := Span(19, 33, 36, 37);
    MidSentenceCommand();
    assert |text| == 52 && End(sp) == 52;
    assert text[End(sp)..] == "";
    NothingAfterTheEnd(true, End(sp));
    assert Shift(sp, 0) == sp;
    assert RegenScan(text) == [Block("b", sp, true)];
  }


  // A multi-line value, and an inline block that shares its line: the block cannot take the
  // value in either form. Written inline, the value's newline would break the line and the
  // next run would read something else; written as the block form, the open tag would start
  // mid-sentence and be prose. `update_regen` leaves the block as it is, and so does the
  // model. `multi-line-into-an-inline-block` in the parity fixtures is this document.
  lemma {:isolate_assertions} ABlockThatSharesItsLineIsLeftAlone()
    ensures var text := "x<!-- REGEN: a -->1<!-- /REGEN -->";
            && RegenSpan(text, "a") == Some(Span(1, 15, 18, 19))
            && UpdateRegen(text, "a", "p\nq") == text
  {
    var text := "x<!-- REGEN: a -->1<!-- /REGEN -->";
    SharedLineDocument();
    TagShapes();
    assert !SubstringAt(text, RegenTag, 0) by { assert text[0..11][0] == text[0]; }
    assert FindFrom(text, RegenTag, 0) == Some(1);
    assert !SubstringAt(text, RegenArrow, 12) by { assert text[12..15][0] == text[12]; }
    assert !SubstringAt(text, RegenArrow, 13) by { assert text[13..16][0] == text[13]; }
    assert !SubstringAt(text, RegenArrow, 14) by { assert text[14..17][0] == text[14]; }
    assert FindFrom(text, RegenArrow, 12) == Some(15);
    assert !SubstringAt(text, RegenClose, 18) by { assert text[18..33][0] == text[18]; }
    assert FindFrom(text, RegenClose, 18) == Some(19);
    var sp := Span(1, 15, 18, 19);
    assert ReadLine(text, false) == InlineBlock(sp);
    OneLine(text, false, sp);
    assert text[12..15] == " a ";
    TrimAroundOne('a');
    assert CommandAt(text, sp) == "a";
    OneBlock(text, false, sp, "a", "p\nq");
    assert InlineAt(text, sp) by { assert text[18..19] == "1"; }
    assert !NoNewline("p\nq") by { assert "p\nq"[1] == '\n'; }
    LineStartIs(text, 1, 0);
    assert text[0..1] == "x";
    assert !StandsAlone(text, false, sp);
    assert Mid(text, false, sp, "a", "p\nq") == text[18..19];
    assert text[..18] + text[18..19] + text[19..] == text;
  }

  // ── what this model does not yet say ──────────────────────────────────────────
  //
  // `scan_markers` reports why it rejected a candidate — `CloseTagMissing`,
  // `ClosedOnAnothersTag` — and this scan only skips it. Every clause above is about the
  // blocks both of them read, and the rejections are what `--check` reports. The model says
  // nothing about the rejections themselves. The TEMPLATE-line check that runs alongside the
  // scan is not modelled either.

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
