"""RFC-0032's reference grammar: one name for a thing, and one parser that reads it.

::

    identifier   yidam://<corpus>/<kind>/<path>[@<rev>][#<property-path>]
    relative     <kind>/<path>  |  <path>          resolved against the containing corpus
    kind         node | crate | catalog | skill | decision

A corpus node had eleven string forms across the yidam repository before this module. Two could
say which corpus a node came from, one could say which revision, and none could say both — they
were what different surfaces invented independently because there was no first form to reuse.

**The parser is total.** RFC-0032 was written on the claim that ``name-not-a-slug`` made a name's
character set an invariant. Measured against the sixteen corpora with tracked nodes, fourteen
conform and two do not, and ``yidam lint --bless`` is the supported way to be one of the two
(#777). So this module parses structurally for any input and answers :func:`reference_conforms`
separately, rather than refusing what it was handed.
"""

from dataclasses import dataclass
from enum import Enum


class Kind(Enum):
    """What a reference names. The ``<kind>`` slot of the grammar.

    A closed vocabulary. ``NODE`` is the default for a relative reference, because every relative
    form in the population it replaced named a node.

    ``ISSUE`` was added on measurement (#783): 252 of the 496 mechanically resolvable references
    across the corpora that write evidence-tag details are issue references — ``[verified — #362]``
    — the single largest kind, and it had no form here.
    """

    NODE = "node"
    CRATE = "crate"
    CATALOG = "catalog"
    SKILL = "skill"
    DECISION = "decision"
    ISSUE = "issue"

    @property
    def path_arity(self) -> int:
        """How many segments this kind's ``<path>`` has.

        Two for a node — ``<class>/<name>`` — and one for everything else. Not a tidiness rule:
        it is what makes the relative form unambiguous, because reading a leading kind word as a
        kind has to leave exactly that many segments behind. See :func:`parse_reference`.
        """
        return 2 if self is Kind.NODE else 1


def kind_from_word(word: str) -> Kind | None:
    """The kind a segment names, or ``None`` if it names no kind."""
    try:
        return Kind(word)
    except ValueError:
        return None


@dataclass(frozen=True)
class Reference:
    """One reference, parsed.

    ``rev`` is a pin and not identity — RFC-0032 §4.3. ``x`` and ``x@abc`` denote the same node in
    two states, which is the split ``ExternalCitation`` already made by holding ``node`` and
    ``commit`` as two fields.

    ``fragment`` names a declared property path and **never a claim**. RFC-0008 measured 22 claims
    entering corpora in resolution commits and 0 matching byte-identically at any participating
    tip, because a sentence search that ends at ``\\n`` extracts one claim as two different strings
    in two hard-wrapped nodes. Identity by surface form is identity by line wrapping.
    """

    corpus: str | None
    kind: Kind
    path: str
    rev: str | None
    fragment: str | None

    @property
    def segments(self) -> list[str]:
        """The ``<path>`` split into its segments."""
        return self.path.split("/") if self.path else []


def is_slug(s: str) -> bool:
    """Whether a segment is a slug: lowercase ASCII words joined by single hyphens.

    It is what fourteen of the sixteen measured corpora already write, and what two do not: a
    manufacturing corpus writes ``NonConformance`` and a water-quality one ``DischargePoint``,
    both taking class names from their domain's vocabulary in its own case. Kebab-case is a
    convention corpora converge on with maturity, not one they start with (#777).

    Written as an explicit character test rather than ``str.islower()`` or a regex, because
    ``islower()`` is true of non-ASCII lowercase and ``\\w`` admits the underscore, and the three
    languages would then be agreeing by coincidence.
    """
    if not s or s.startswith("-") or s.endswith("-") or "--" in s:
        return False
    return all(("a" <= c <= "z") or ("0" <= c <= "9") or c == "-" for c in s)


def reference_conforms(r: Reference) -> bool:
    """Whether every segment of a reference is a slug, so it needs no escaping in any rendering.

    The separate predicate RFC-0032 §4.6 calls for. A caller that must emit a URI acts on the
    answer; the parser does not refuse on its behalf. This is the test ``export_rdf.rs`` already
    applies to a foreign alignment IRI — check whether it is dereferenceable and demote it when it
    is not — turned inward onto our own identifiers, which §2 of that RFC complains it never was.
    """
    if r.corpus is not None and not is_slug(r.corpus):
        return False
    if not r.path:
        return False
    segments = r.segments
    if not all(is_slug(s) for s in segments):
        return False
    if len(segments) != r.kind.path_arity:
        return False
    if r.rev is not None and not is_slug(r.rev):
        return False
    if r.fragment is not None and not fragment_conforms(r.kind, r.fragment):
        return False
    return True


def fragment_conforms(kind: Kind, fragment: str) -> bool:
    """Whether a fragment names something, for the kind it is attached to.

    **Two rules.** For every kind but ``CRATE`` it is a declared property path — dotted slugs —
    which is §4.4 unchanged. For a ``CRATE`` it names a code item or a file inside that crate,
    named by a foreign toolchain: ``ohio_panel::equalization_by_year`` and
    ``tests/the_weights_behind_the_index.rs`` are both real references in a measured corpus, and
    neither is a dotted slug path.

    §4.4's refusal survives. What it refuses is a fragment naming a **claim**, because RFC-0008
    measured that a claim has no identity by surface form. A Rust item path has one a compiler
    enforces and a file path one the filesystem does; the argument does not reach either.

    The crate rule admits uppercase and ``.``, which :func:`is_slug` does not — a type is
    ``CamelCase`` and a file has an extension, and neither is this project's naming rule to make.
    """
    if not fragment:
        return False
    if kind is not Kind.CRATE:
        return all(is_slug(p) for p in fragment.split("."))
    for part in fragment.split("/"):
        for seg in part.split("::"):
            if not seg:
                return False
            if not all(
                ("a" <= c <= "z") or ("A" <= c <= "Z") or ("0" <= c <= "9") or c in "_-."
                for c in seg
            ):
                return False
    return True


def _split_authority(body: str) -> tuple[str | None, str]:
    """Peel the naming corpus off the front, in whichever of the three ways it can be written.

    ``yidam://<corpus>/…`` is the grammar's. ``yidam://corpus/…`` is RFC-0005's frozen authority,
    where ``corpus`` is a collection kind rather than a corpus name — the defect RFC-0032 §1 is
    about — and it means *this* corpus, so it maps to ``None``. ``pkg::class/name`` is
    ``qualified_id``'s form, the only string in the eleven that could say which corpus.
    """
    prefix = "yidam://"
    if body.startswith(prefix):
        rest = body[len(prefix) :]
        authority, _, tail = rest.partition("/")
        if authority in ("corpus", ""):
            return None, tail
        # `skills`/`decisions` spend the authority on the *kind*, so the word moves into the path
        # where the kind split reads it, and the two aliases need no second reader.
        if authority == "skills":
            return None, f"skill/{tail}"
        if authority == "decisions":
            return None, f"decision/{tail}"
        return authority, tail
    pkg, sep, rest = body.partition("::")
    if sep and pkg and rest:
        return pkg, rest
    return None, body


def _normalise_node_path(rest: str) -> str:
    """The node-path spellings the repository already wrote, reduced to ``<class>/<name>``.

    ``find_node`` tolerated three of these and no contract mentioned any of them. An ad-hoc
    resolver is what an absent grammar looks like from the inside, so they are admitted here,
    once, instead of in each surface.
    """
    out = rest.lstrip("/")
    marker = ".yidam/corpus/"
    at = out.find(marker)
    if at != -1:
        out = out[at + len(marker) :]
    if out.endswith("/"):
        out = out[:-1]
    if out.endswith(".yml"):
        out = out[: -len(".yml")]
    return out


def _split_kind(rest: str) -> tuple[Kind, str] | None:
    """Split the kind off the front of a path, defaulting to ``NODE``."""
    cleaned = _normalise_node_path(rest)
    if not cleaned:
        return None
    count = len(cleaned.split("/"))
    head, sep, tail = cleaned.partition("/")
    if sep:
        kind = kind_from_word(head)
        # The arity test. Reading `head` as a kind must leave exactly that kind's path, or `head`
        # was a class name and the whole string is the path.
        if kind is not None and count - 1 == kind.path_arity:
            return kind, tail
    return Kind.NODE, cleaned


def parse_reference(text: str) -> Reference | None:
    """Parse a reference. ``None`` only when the input names no thing at all.

    Accepts the canonical ``yidam://<corpus>/<kind>/<path>`` identifier, the relative forms, and
    the legacy spellings the repository already wrote — a trailing ``.yml``, a full
    ``.yidam/corpus/<class>/<name>.yml`` path, ``pkg::class/name``, and RFC-0005's frozen resource
    URIs.

    **The relative form is unambiguous, by arity.** ``<kind>/<path>`` and a bare ``<path>`` share
    a shape, and a corpus may legitimately declare a class named ``node``. The path's segment
    count decides: ``node/concept/foo`` is the node ``concept/foo``; ``node/foo`` is the node
    ``foo`` in a class called ``node``. Where both readings are valid — ``skill/foo``, with a
    class named ``skill`` — the kind wins, because the grammar puts a kind in that position; and
    :func:`render_reference` emits the explicit ``node/skill/foo`` for the other reading, so the
    round trip is total.

    **``@`` is split from the right and ``#`` from the left.** A conforming segment contains
    neither, so the rule is only visible on input that already failed :func:`reference_conforms`.
    """
    s = text.strip()
    if not s:
        return None

    head, sep, frag = s.partition("#")
    fragment = frag if sep else None
    s = head

    at = s.rfind("@")
    has_rev = 0 < at < len(s) - 1
    rev = s[at + 1 :] if has_rev else None
    body = s[:at] if has_rev else s

    corpus, rest = _split_authority(body)
    split = _split_kind(rest)
    if split is None:
        return None
    kind, path = split

    return Reference(corpus=corpus, kind=kind, path=path, rev=rev, fragment=fragment)


def render_reference(r: Reference) -> str:
    """Render a reference. The only place an identifier is built.

    A corpus renders the canonical ``yidam://`` identifier; its absence renders the relative form.
    The locator — RFC-0032 §4.2's ``https://`` rendering, for anything a stranger has to follow —
    is derived per corpus from a declared base and is not this function's job: an identifier must
    survive a ``.yiz`` tarball, a private corpus and an offline clone, none of which have a host.

    **A relative node reference names its kind when its class would otherwise be read as one.** A
    node in a class called ``skill`` renders ``node/skill/foo``, not ``skill/foo``, so that
    :func:`parse_reference` returns what was rendered. Without that the round trip fails on
    exactly the corpora that name a class after a kind, and silently.
    """
    out = ""
    if r.corpus is not None:
        out += f"yidam://{r.corpus}/{r.kind.value}/"
    else:
        shadows_a_kind = kind_from_word(r.path.split("/")[0]) is not None
        if r.kind is not Kind.NODE or shadows_a_kind:
            out += f"{r.kind.value}/"
    out += r.path
    if r.rev is not None:
        out += f"@{r.rev}"
    if r.fragment is not None:
        out += f"#{r.fragment}"
    return out
