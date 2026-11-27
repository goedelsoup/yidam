"""`prose.of` and `prose.text` trim by Unicode `White_Space`, as the Rust SDK's `str::trim` does.

Held here rather than as a parity fixture: `prose` is not on the parity function list.
`compose_embed_text` reads prose through `prose.of`, so its fixtures pin the emptiness test
(#1699). `prose.text` is the one call no fixture reaches, and this pins both.

Python's `str.strip()` also removes U+001C to U+001F, and neither U+FEFF nor U+0085 differs
from Rust here, so the cases are the ones the built-in gets wrong.
"""

import pytest

from yidam_core import corpus, prose


def _node(value: str) -> corpus.CorpusInstance:
    return corpus.parse_instance(f'class: measure\nlabel: L\ndescription: "{value}"\n')


@pytest.mark.parametrize(
    ("value", "kept"),
    [
        ("\\x1F", True),
        ("\\uFEFF", True),
        ("\\x85", False),
        (" ", False),
    ],
)
def test_a_value_of_only_one_character_is_prose_unless_it_is_white_space(value, kept):
    got = prose.of(_node(value), ["description"], [])
    assert (len(got) == 1) is kept


@pytest.mark.parametrize(
    ("value", "text"),
    [
        ("x\\x1F", "x\x1f"),
        ("x\\uFEFF", "x\ufeff"),
        ("x\\x85", "x"),
        ("x ", "x"),
    ],
)
def test_text_trims_the_end_of_each_value_by_white_space(value, text):
    assert prose.text(_node(value), ["description"], []) == text
