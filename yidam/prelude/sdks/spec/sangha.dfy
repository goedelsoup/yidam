// Dafny specification: sangha resolution soundness.
//
// Two key theorems:
//   Article V — Resolution scope fidelity:
//     No claim appears in the evolution unless at least one elector position held it.
//     ("Resolution is synthesis, not generation.")
//
//   Article III — Provenance completeness:
//     Every contributing elector's tip hash is recorded in the evolution.
//     Every unresolved tension becomes an open question.
//
// Run: dafny verify prelude/sdks/spec/sangha.dfy

module YidamSangha {

  // ── Types ─────────────────────────────────────────────────────────────────────

  datatype Option<T> = None | Some(value: T)

  // A claim held by a sangha position (simplified — full claim type in graph.dfy).
  //
  // `text` is the whole of a claim's identity here, so `ArticleV` below is decided by string
  // equality. That is a modelling choice, not a rule a checker may inherit: measured against a
  // derived repository's 29 resolutions, no claim a resolution introduced appears byte-identically
  // at any participating tip, and the corpus's own extractor returns a different string for one
  // claim depending on where the source line-wraps. The theorems still hold — they are about a
  // sequence of values with decidable equality — but `Claim(text)` is not a claim identity for
  // real commits. CONSTITUTION.md Article V ("On identity") puts that judgement with the
  // synthesizer and leaves the gatable half to nodes and edges; see docs/rfcs/0008-emergent-claims.md.
  datatype Claim = Claim(text: string)

  // A sangha position: one elector's current branch-tip view.
  datatype SanghaPosition = SanghaPosition(
    elector: string,
    branch: string,
    tip_hash: string,
    claims: seq<Claim>
  )

  // A rigpa evolution: the synthesis of all positions.
  datatype SanghaEvolution = SanghaEvolution(
    name: string,
    branch: string,
    tip_hash: string,
    claims: seq<Claim>,          // synthesized claims
    source_hashes: seq<string>,  // contributing elector tip hashes
    open_questions: seq<Claim>   // tensions that remain unresolved
  )

  // A tension exists when two positions hold conflicting claims on the same subject.
  // We model a conflict as a claim present in one position but absent in another.
  predicate Tension(positions: seq<SanghaPosition>, c: Claim) {
    exists i, j ::
      0 <= i < |positions| &&
      0 <= j < |positions| &&
      i != j &&
      c in positions[i].claims &&
      c !in positions[j].claims
  }

  // ── Article V — Resolution scope fidelity ─────────────────────────────────────
  //
  // Every claim in the evolution was held by at least one elector.
  // This is the formal statement that resolution is synthesis, not generation.

  predicate ArticleV(positions: seq<SanghaPosition>, evo: SanghaEvolution) {
    forall c :: c in evo.claims ==>
      exists i :: 0 <= i < |positions| && c in positions[i].claims
  }

  // The union synthesis satisfies Article V by construction.
  function UnionSynthesis(name: string, positions: seq<SanghaPosition>): SanghaEvolution {
    SanghaEvolution(
      name := name,
      branch := "refs/heads/rigpa/" + name,
      tip_hash := "",     // set at commit time
      claims := CollectClaims(positions),
      source_hashes := CollectHashes(positions),
      open_questions := []
    )
  }

  function CollectClaims(positions: seq<SanghaPosition>): seq<Claim>
    decreases |positions|
  {
    if |positions| == 0 then []
    else positions[0].claims + CollectClaims(positions[1..])
  }

  function CollectHashes(positions: seq<SanghaPosition>): seq<string>
    decreases |positions|
  {
    if |positions| == 0 then []
    else [positions[0].tip_hash] + CollectHashes(positions[1..])
  }

  lemma CollectClaimsCovers(positions: seq<SanghaPosition>, c: Claim)
    requires exists i :: 0 <= i < |positions| && c in positions[i].claims
    ensures c in CollectClaims(positions)
    decreases |positions|
  {
    if |positions| > 0 {
      var i :| 0 <= i < |positions| && c in positions[i].claims;
      if i == 0 {
        assert c in positions[0].claims;
      } else {
        CollectClaimsCovers(positions[1..], c);
      }
    }
  }

  lemma CollectClaimsSound(positions: seq<SanghaPosition>, c: Claim)
    requires c in CollectClaims(positions)
    ensures exists i :: 0 <= i < |positions| && c in positions[i].claims
    decreases |positions|
  {
    if |positions| > 0 {
      if c in positions[0].claims {
        assert 0 < |positions| && c in positions[0].claims;
      } else {
        CollectClaimsSound(positions[1..], c);
        var j :| 0 <= j < |positions[1..]| && c in positions[1..][j].claims;
        assert 0 <= j + 1 < |positions| && c in positions[j + 1].claims;
      }
    }
  }

  // Article V holds for union synthesis.
  lemma UnionSynthesisArticleV(name: string, positions: seq<SanghaPosition>)
    ensures ArticleV(positions, UnionSynthesis(name, positions))
  {
    var evo := UnionSynthesis(name, positions);
    forall c | c in evo.claims ensures exists i :: 0 <= i < |positions| && c in positions[i].claims {
      CollectClaimsSound(positions, c);
    }
  }

  // ── Article III — Provenance completeness ─────────────────────────────────────
  //
  // Every contributing elector's tip hash appears in the evolution's source list.
  // Every tension that was not resolved becomes an open question.

  predicate ArticleIII_Hashes(positions: seq<SanghaPosition>, evo: SanghaEvolution) {
    forall i :: 0 <= i < |positions| ==> positions[i].tip_hash in evo.source_hashes
  }

  ghost predicate ArticleIII_OpenQuestions(positions: seq<SanghaPosition>, evo: SanghaEvolution) {
    forall c :: Tension(positions, c) && c !in evo.claims ==> c in evo.open_questions
  }

  // Hashes collected from all positions are all present in source_hashes.
  lemma CollectHashesCover(positions: seq<SanghaPosition>, i: int)
    requires 0 <= i < |positions|
    ensures positions[i].tip_hash in CollectHashes(positions)
    decreases |positions|
  {
    if i == 0 {
      assert positions[0].tip_hash in CollectHashes(positions);
    } else {
      CollectHashesCover(positions[1..], i - 1);
      assert positions[i].tip_hash in CollectHashes(positions[1..]);
    }
  }

  lemma UnionSynthesisArticleIII(name: string, positions: seq<SanghaPosition>)
    ensures ArticleIII_Hashes(positions, UnionSynthesis(name, positions))
  {
    forall i | 0 <= i < |positions|
      ensures positions[i].tip_hash in UnionSynthesis(name, positions).source_hashes
    {
      CollectHashesCover(positions, i);
    }
  }

  // ── Resolution soundness — combined ───────────────────────────────────────────
  //
  // A resolution is sound when it satisfies both Article V and Article III (hashes).
  // The union synthesis is always sound; implementations must be no weaker.

  predicate ResolutionSound(positions: seq<SanghaPosition>, evo: SanghaEvolution) {
    ArticleV(positions, evo) && ArticleIII_Hashes(positions, evo)
  }

  lemma UnionSynthesisIsSound(name: string, positions: seq<SanghaPosition>)
    ensures ResolutionSound(positions, UnionSynthesis(name, positions))
  {
    UnionSynthesisArticleV(name, positions);
    UnionSynthesisArticleIII(name, positions);
  }

  // A stronger resolution (one that resolves tensions into specific claims rather than
  // carrying all of them) is also sound, provided it holds Article V for every claim
  // it includes and records all contributing hashes.
  lemma StrongerResolutionIsSound(
    positions: seq<SanghaPosition>,
    evo: SanghaEvolution
  )
    requires ArticleV(positions, evo)
    requires ArticleIII_Hashes(positions, evo)
    ensures ResolutionSound(positions, evo)
  {}
}
