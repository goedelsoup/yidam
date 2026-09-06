-- Yidam core: type-theoretic corpus and resolution model.
--
-- Three structural claims, and what this file actually establishes about each:
--   (1) Corpus graph forms a category: nodes as objects, link-paths as morphisms.  PROVED
--   (2) Sangha positions form a partial order under semantic entailment.           PROVED
--   (3) Constitutional non-contradiction: domain augmentations are purely additive. PROVED
--
-- (3) was `True`, proved by `trivial`, from three unused hypotheses — a claim that would
-- still have elaborated if every article of the constitution were false. #499 gave it a
-- conclusion. It now says what its own comment always said it meant: that an augmentation
-- may only add, and that adding cannot withdraw a claim or lose a morphism. `ConstitutionBase`
-- lost its `True` placeholders in the same pass, because a theorem about a structure of
-- tautologies is a tautology one level up.
--
-- The build is warning-free, and that is the check: the three `unused variable` warnings that
-- used to sit on a green build were Lean reporting, on every run, that the hypotheses did no
-- work. A hypothesis that earns its place is one the proof cannot be completed without.
--
-- Run: mise run verify   (or `lake build Yidam` from this directory)

-- ── Core types ────────────────────────────────────────────────────────────────

inductive EvidenceTag
  | Verified
  | Inference
  | Open
  | Implicit
  deriving Repr, DecidableEq

inductive CommitKind
  | Epistemic
  | Operational
  deriving Repr, DecidableEq

structure Claim where
  text : String
  tag  : EvidenceTag
  deriving Repr, DecidableEq

structure Link where
  label  : String
  target : String
  anchor : Option String := none
  deriving Repr

structure CorpusNode where
  path   : String
  title  : String
  claims : List Claim
  links  : List Link
  deriving Repr

-- ── Corpus graph as a category ────────────────────────────────────────────────
--
-- Objects: CorpusNode (indexed by path)
-- Morphisms: sequences of direct link steps
-- Identity: the empty path (a node linking to itself via zero steps)
-- Composition: concatenation of link paths

abbrev CorpusGraph := List (String × CorpusNode)

def lookupNode (g : CorpusGraph) (path : String) : Option CorpusNode :=
  (g.find? (fun p => p.1 == path)).map (·.2)

-- A morphism from `src` to `dst` is a proof that dst is reachable from src.
inductive Reachable : CorpusGraph → String → String → Prop where
  | refl  : lookupNode g src ≠ none → Reachable g src src
  | step  : Reachable g src mid
          → (∃ node, lookupNode g mid = some node
                   ∧ node.links.any (fun l => l.target == dst))
          → Reachable g src dst

-- Identity morphism: every node reaches itself.
theorem corpus_id (g : CorpusGraph) (src : String) (h : lookupNode g src ≠ none) :
    Reachable g src src :=
  Reachable.refl h

-- Composition: reachability is transitive.
theorem corpus_comp (g : CorpusGraph) (a b c : String)
    (hab : Reachable g a b) (hbc : Reachable g b c) : Reachable g a c := by
  induction hbc with
  | refl _      => exact hab
  | step _ s ih => exact Reachable.step ih s

-- ── DAG condition ──────────────────────────────────────────────────────────────
--
-- A corpus is acyclic when no pair of distinct nodes are mutually reachable.
-- This is the structural invariant that makes the graph a thin category.

def IsDAG (g : CorpusGraph) : Prop :=
  ∀ (u v : String), u ≠ v → ¬ (Reachable g u v ∧ Reachable g v u)

-- In a DAG, the only path from a node to itself is the identity.
--
-- `¬(A ∧ B) → ¬A ∨ ¬B` is classical, so the proof needs excluded middle — but it needs only
-- that. The version that stood here reached for `by_contra`/`push_neg`, which are Mathlib
-- tactics this package does not depend on and never has, so it did not elaborate. Mathlib
-- for one De Morgan step would be the largest dependency in the repository; `Classical.em`
-- is in core and proves the same statement, unweakened.
theorem dag_no_nontrivial_cycle (g : CorpusGraph) (hdag : IsDAG g) (v : String) :
    ∀ (u : String), u ≠ v → ¬ Reachable g v u ∨ ¬ Reachable g u v :=
  fun u huv =>
    (Classical.em (Reachable g v u)).elim
      (fun hvu => Or.inr (fun huv' => hdag u v huv ⟨huv', hvu⟩))
      Or.inl

-- ── Sangha positions as a partial order ──────────────────────────────────────
--
-- A position P is weaker than Q (P ≤ Q) when every claim P holds is also held
-- by Q. Semantically: Q "knows at least as much" as P on every axis where P
-- has taken a stance.

structure SanghaPosition where
  elector : String
  branch  : String
  tip     : String
  claims  : List Claim
  deriving Repr

def PositionLE (p q : SanghaPosition) : Prop :=
  ∀ c, c ∈ p.claims → c ∈ q.claims

-- Reflexivity.
theorem pos_le_refl (p : SanghaPosition) : PositionLE p p :=
  fun _ h => h

-- Transitivity.
theorem pos_le_trans (p q r : SanghaPosition)
    (hpq : PositionLE p q) (hqr : PositionLE q r) : PositionLE p r :=
  fun c h => hqr c (hpq c h)

-- Antisymmetry (of the claim-set relation): if p ≤ q and q ≤ p, the positions
-- hold exactly the same claims (though they may differ in elector or hash).
theorem pos_le_antisymm_claims (p q : SanghaPosition)
    (hpq : PositionLE p q) (hqp : PositionLE q p) :
    ∀ c, c ∈ p.claims ↔ c ∈ q.claims :=
  fun c => ⟨hpq c, hqp c⟩

-- ── Rigpa synthesis as join ────────────────────────────────────────────────────
--
-- The join of a set of positions is the smallest evolution that dominates all
-- of them under PositionLE. The union synthesis is the canonical such join.

structure SanghaEvolution where
  name    : String
  claims  : List Claim
  sources : List String   -- contributing elector tip hashes
  deriving Repr

-- Article V: scope fidelity — every claim in the evolution was held by
-- at least one elector. (Resolution is synthesis, not generation.)
def ArticleV (positions : List SanghaPosition) (evo : SanghaEvolution) : Prop :=
  ∀ c, c ∈ evo.claims → ∃ p ∈ positions, c ∈ p.claims

-- Article III: provenance — every elector's tip hash is cited.
def ArticleIII (positions : List SanghaPosition) (evo : SanghaEvolution) : Prop :=
  ∀ p ∈ positions, p.tip ∈ evo.sources

-- The union synthesis: all claims from all positions.
def unionSynthesis (name : String) (positions : List SanghaPosition) : SanghaEvolution :=
  { name    := name
  , claims  := positions.bind (·.claims)
  , sources := positions.map (·.tip)
  }

-- Union synthesis satisfies Article V.
theorem union_satisfies_article_v (name : String) (positions : List SanghaPosition) :
    ArticleV positions (unionSynthesis name positions) := by
  intro c hc
  simp [unionSynthesis, List.mem_bind] at hc
  obtain ⟨p, hp, hcp⟩ := hc
  exact ⟨p, hp, hcp⟩

-- Union synthesis satisfies Article III.
theorem union_satisfies_article_iii (name : String) (positions : List SanghaPosition) :
    ArticleIII positions (unionSynthesis name positions) := by
  intro p hp
  simp [unionSynthesis, List.mem_map]
  exact ⟨p, hp, rfl⟩

-- ── Constitutional non-contradiction ─────────────────────────────────────────
--
-- The constitution is a set of structural invariants (Articles I–VI). A domain augmentation
-- adds material: nodes to the corpus, evolutions to the record, articles of its own. Claim
-- (3) is that adding material cannot withdraw what the base already established.
--
-- What stood here asserted `True` from unused hypotheses, over a `ConstitutionBase` whose
-- three interesting fields were also `True`. Both halves are below with content, and the
-- articles are stated over the data they constrain rather than over nothing.

-- Article II: classification is total.
--
-- The verb sets themselves are in `graph.dfy`, which all three SDKs cite as their source; a
-- fourth copy here would be a fourth thing to keep in step, and the drift `graph.dfy` already
-- records (a spec missing "fix" and "regen" that every implementation carried) is what that
-- costs. What Lean adds is the shape: classification is a *total function* into a
-- two-constructor type, so there is no verb it declines and no third outcome to handle.
-- `ClassifyCommitTotal` in graph.dfy is the same claim about the concrete function.
def ClassificationTotal (classify : String → CommitKind) : Prop :=
  ∀ verb, classify verb = CommitKind.Epistemic ∨ classify verb = CommitKind.Operational

theorem classification_is_total (classify : String → CommitKind) :
    ClassificationTotal classify := by
  intro verb
  cases classify verb with
  | Epistemic   => exact Or.inl rfl
  | Operational => exact Or.inr rfl

-- The base constitution, over the data each article is about: the corpus graph, the elector
-- positions, the evolutions on record, and the commit classifier.
structure ConstitutionBase
    (g : CorpusGraph)
    (positions : List SanghaPosition)
    (evolutions : List SanghaEvolution)
    (classify : String → CommitKind) : Prop where
  -- Article II: every commit verb receives a kind. There is no unclassified commit.
  commitsTotal     : ClassificationTotal classify
  -- Article III: every evolution on record cites every elector's tip.
  provenanceSound  : ∀ evo ∈ evolutions, ArticleIII positions evo
  -- Article V: every evolution on record synthesises rather than generates.
  evolutionsScoped : ∀ evo ∈ evolutions, ArticleV positions evo
  -- Article VI: the corpus graph is acyclic.
  corpusIsDAG      : IsDAG g

-- ── What "purely additive" means ─────────────────────────────────────────────
--
-- The comment that stood beside the theorem said it: an augmentation "MUST NOT modify claims
-- already in the corpus or contradict established evolutions — they may only add". That is a
-- statement about two corpus graphs, and this is it.

/-- `g'` augments `g` when every node `g` holds, `g'` holds **unchanged**. Nodes may be
    added; an existing node's claims and links may be neither modified nor removed. -/
def AugmentsGraph (g g' : CorpusGraph) : Prop :=
  ∀ path node, lookupNode g path = some node → lookupNode g' path = some node

/-- Nothing reachable becomes unreachable: every morphism of the base category is a morphism
    of the augmented one. This is the categorical content of "purely additive", and it is
    where the hypothesis does the work — the previous statement's hypotheses did none. -/
theorem augmentation_preserves_morphisms {g g' : CorpusGraph} (h : AugmentsGraph g g') :
    ∀ a b, Reachable g a b → Reachable g' a b := by
  intro a b hab
  induction hab with
  | refl hsrc =>
      cases hg : lookupNode g a with
      | none   => exact absurd hg hsrc
      | some n => exact Reachable.refl (by rw [h a n hg]; simp)
  | step _ hstep ih =>
      obtain ⟨n, hn, hlinks⟩ := hstep
      exact Reachable.step ih ⟨n, h _ n hn, hlinks⟩

/-- And no claim is retracted: a node the base corpus held is held by the augmented one with
    the same claims on it. -/
theorem augmentation_retracts_no_claim {g g' : CorpusGraph} (h : AugmentsGraph g g') :
    ∀ path node, lookupNode g path = some node →
      ∃ node', lookupNode g' path = some node' ∧ node'.claims = node.claims :=
  fun path node hn => ⟨node, h path node hn, rfl⟩

-- ── Article VI is an obligation, not an inheritance ──────────────────────────
--
-- `IsDAG g` was a hypothesis of the theorem this replaces, and Lean reported it unused on
-- every build. That is not an accident of the proof: acyclicity of the base says nothing
-- about the augmentation, because adding nodes is exactly how a cycle appears. The witness
-- is two nodes pointing at each other, added to a corpus that had none.

private def cycleA : CorpusNode :=
  { path := "a", title := "A", claims := [], links := [{ label := "cites", target := "b" }] }

private def cycleB : CorpusNode :=
  { path := "b", title := "B", claims := [], links := [{ label := "cites", target := "a" }] }

private def cyclicPair : CorpusGraph := [("a", cycleA), ("b", cycleB)]

/-- The empty corpus reaches nothing, so it is vacuously acyclic. -/
theorem not_reachable_in_empty (a b : String) : ¬ Reachable [] a b := by
  intro h
  cases h with
  | refl hsrc      => exact hsrc (by simp [lookupNode])
  | step _ hstep   => obtain ⟨_, hn, _⟩ := hstep; simp [lookupNode] at hn

theorem additivity_does_not_preserve_acyclicity :
    ∃ (g g' : CorpusGraph), AugmentsGraph g g' ∧ IsDAG g ∧ ¬ IsDAG g' := by
  refine ⟨[], cyclicPair, ?_, ?_, ?_⟩
  · intro path node hn
    simp [lookupNode] at hn
  · intro u v _ hboth
    exact not_reachable_in_empty u v hboth.1
  · intro hdag
    have hab : Reachable cyclicPair "a" "b" :=
      Reachable.step (Reachable.refl (by simp [lookupNode, cyclicPair]))
        ⟨cycleA, by simp [lookupNode, cyclicPair], by simp [cycleA]⟩
    have hba : Reachable cyclicPair "b" "a" :=
      Reachable.step (Reachable.refl (by simp [lookupNode, cyclicPair]))
        ⟨cycleB, by simp [lookupNode, cyclicPair], by simp [cycleB]⟩
    exact hdag "a" "b" (by simp) ⟨hab, hba⟩

-- ── The theorem ──────────────────────────────────────────────────────────────

/-- What an augmentation owes for the material it adds.

    Article VI is here rather than inherited, for the reason
    `additivity_does_not_preserve_acyclicity` proves. Articles III and V are here for the new
    evolutions only: the base already established them for the ones on record, and the theorem
    below carries that across rather than asking for it again.

    `faithful` rather than `scoped`, because `scoped` is a Lean keyword and a field of that
    name does not parse. -/
structure AugmentationObligations
    (g' : CorpusGraph) (positions : List SanghaPosition)
    (added : List SanghaEvolution) : Prop where
  acyclic : IsDAG g'
  cited   : ∀ evo ∈ added, ArticleIII positions evo
  faithful : ∀ evo ∈ added, ArticleV positions evo

/-- An augmentation is consistent when the augmented constitution has a model — when
    `ConstitutionBase` at the augmented data is inhabited. Deriving `False` from the articles
    is exactly what would leave it empty. -/
def AugmentationConsistent
    (g : CorpusGraph) (positions : List SanghaPosition)
    (evolutions : List SanghaEvolution) (classify : String → CommitKind) : Prop :=
  Nonempty (ConstitutionBase g positions evolutions classify)

/-- **Claim (3).** A base constitution, plus a purely additive corpus augmentation, plus an
    augmentation that discharges its own obligations, is a constitution: no article is
    contradicted, and nothing the base established is withdrawn.

    Read the three conjuncts as the three things "non-contradiction" was always meant to say:
    the augmented articles all hold; no node's claims were modified; no morphism was lost. -/
theorem additive_augmentations_do_not_contradict
    {g g' : CorpusGraph} {positions : List SanghaPosition}
    {evolutions added : List SanghaEvolution} {classify : String → CommitKind}
    (base : ConstitutionBase g positions evolutions classify)
    (hgraph : AugmentsGraph g g')
    (obligations : AugmentationObligations g' positions added) :
    ConstitutionBase g' positions (evolutions ++ added) classify
    ∧ (∀ path node, lookupNode g path = some node →
        ∃ node', lookupNode g' path = some node' ∧ node'.claims = node.claims)
    ∧ (∀ a b, Reachable g a b → Reachable g' a b) := by
  refine ⟨⟨base.commitsTotal, ?_, ?_, obligations.acyclic⟩, ?_, ?_⟩
  · intro evo hevo
    rcases List.mem_append.mp hevo with h | h
    · exact base.provenanceSound evo h
    · exact obligations.cited evo h
  · intro evo hevo
    rcases List.mem_append.mp hevo with h | h
    · exact base.evolutionsScoped evo h
    · exact obligations.faithful evo h
  · exact augmentation_retracts_no_claim hgraph
  · exact augmentation_preserves_morphisms hgraph

/-- The consistency corollary, which is the form the claim is usually quoted in: an additive
    augmentation cannot make the constitution unsatisfiable. -/
theorem additive_augmentations_are_consistent
    {g g' : CorpusGraph} {positions : List SanghaPosition}
    {evolutions added : List SanghaEvolution} {classify : String → CommitKind}
    (base : ConstitutionBase g positions evolutions classify)
    (hgraph : AugmentsGraph g g')
    (obligations : AugmentationObligations g' positions added) :
    AugmentationConsistent g' positions (evolutions ++ added) classify :=
  ⟨(additive_augmentations_do_not_contradict base hgraph obligations).1⟩
