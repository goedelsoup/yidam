"""The calculator kit's writer, held to PyYAML (RFC-0056 §2.5).

`yidam`'s own tests hold the kit to `serde_yaml`. A calculator's author reads the same file
with PyYAML, which follows YAML 1.1 where `serde_yaml` follows 1.2: `yes`, `00:00` and `1_000`
are a bool, a sexagesimal int and an int to one and a string to the other. The kit quotes every
string so both read it as what it was given. This holds it to that against the hostile fixture.
"""

import json
import math
import pathlib
import subprocess
import sys

import yaml

REPO = pathlib.Path(__file__).resolve().parents[4]
KIT = REPO / "yidam/prelude/kit"
HOSTILE = REPO / "yidam/cli/tests/fixtures/calculator-kit/hostile.json"

sys.path.insert(0, str(KIT))
import yidam_kit  # noqa: E402


def hostile():
    return json.loads(HOSTILE.read_text(encoding="utf-8"))


def same(a, b):
    """Equal, with -0.0 kept apart from 0.0 and an int kept apart from a float."""
    if isinstance(a, float) or isinstance(b, float):
        return (
            type(a) is type(b)
            and (a == b or (math.isnan(a) and math.isnan(b)))
            and math.copysign(1, a) == math.copysign(1, b)
        )
    if isinstance(a, list):
        return isinstance(b, list) and len(a) == len(b) and all(map(same, a, b))
    if isinstance(a, dict):
        return isinstance(b, dict) and a.keys() == b.keys() and all(same(a[k], b[k]) for k in a)
    return type(a) is type(b) and a == b


def test_every_hostile_value_reads_back_in_pyyaml_as_itself():
    d = hostile()
    for value in d["values"] + [d]:
        text = yidam_kit.dump(value)
        back = yaml.safe_load(text)
        assert same(back, value), text


def test_a_signal_table_reads_back_in_pyyaml_with_its_rows():
    d = hostile()
    text = yidam_kit.signals(d["rows"], method={"rule": "hostile"})
    back = yaml.safe_load(text)
    assert back["format_version"] == 1
    assert same(back["signals"], d["rows"]), text


def test_the_kit_imports_in_isolated_mode_with_nothing_but_the_standard_library():
    probe = (
        "import sys; sys.path.insert(0, sys.argv[1]); import yidam_kit; "
        "assert 'yaml' not in sys.modules, 'the kit imported PyYAML'"
    )
    subprocess.run([sys.executable, "-I", "-c", probe, str(KIT)], check=True)
