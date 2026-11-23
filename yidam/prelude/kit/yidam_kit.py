"""yidam_kit: read the graph `yidam run` hands a step, and write a file it will read back.

RFC-0056 §2.2. A calculator is invoked with the resolved corpus at `$YIDAM_GRAPH`, in the
tab-separated records `yidam run` writes, and is expected to write YAML that `yidam` reads.
Both halves are small and both have gone wrong in the same ways in more than one corpus: a
reader that dropped a property's kind, and a writer that wrote `- [00:00, 184, 0.5]`, which
PyYAML refuses. This module is both halves, once.

Standard library only, Python 3.9 or later, so it runs under `python3 -I`. A step that uses it
lists `.yidam/.vendor/prelude/kit/**` in its `reads`, which also makes a kit update a reason
for the step to re-run, and imports it from under `$YIDAM_IN`:

    import os, sys
    sys.path.insert(0, os.path.join(os.environ["YIDAM_IN"], ".yidam/.vendor/prelude/kit"))
    import yidam_kit

    graph = yidam_kit.read()
    rows = [{"node": n.ref, "links": len(n.links)} for n in graph.nodes]
    text = yidam_kit.signals(rows, method={"rule": "count outgoing links"})

The kit does not know `yidam`'s rules for a signal table: which names are reserved, and which
nodes exist. `yidam run` checks those against the corpus before it commits, and refuses the
step with nothing committed if a row breaks one.

A step that reads a table a catalog entry declares reads it with `table(path)`, which types its
cells by the declaration `yidam run` names in `$YIDAM_TABLES` (RFC-0059 §2.4).

`yidam_kit.rs` is the same kit in Rust. The two are held to one another's output by the
template's tests, byte for byte on the writer's side.
"""

import csv
import io
import json
import math
import os
import re

GRAPH_FORMAT_VERSION = 1
"""The `$YIDAM_GRAPH` record grammar this kit reads. Another version is refused."""

SIGNALS_FORMAT_VERSION = 1
"""The signal-table format `signals()` writes."""


class KitError(ValueError):
    """A graph this kit cannot read, or a value it will not write."""


# -- reading ---------------------------------------------------------------------------------


class Opaque:
    """A property the graph states the kind of and gives no value for.

    The graph flattens a list of scalars and a list of records. Anything deeper (a mapping at
    the top level, a list inside a list, a record field that is itself nested) is one row that
    names its kind and stops. The value is in the node's file.
    """

    __slots__ = ("kind",)

    def __init__(self, kind):
        self.kind = kind

    def __eq__(self, other):
        return isinstance(other, Opaque) and other.kind == self.kind

    def __repr__(self):
        return "Opaque(%r)" % self.kind


class Row:
    """One `prop` record as the graph carried it: the key, the kind, and the decoded value.

    A `seq` row's value is the item count, as the graph writes it; the list is the property.
    """

    __slots__ = ("key", "kind", "value")

    def __init__(self, key, kind, value):
        self.key = key
        self.kind = kind
        self.value = value

    def __repr__(self):
        return "Row(%r, %r, %r)" % (self.key, self.kind, self.value)


class Link:
    """One outgoing link, resolved by `yidam`. `exists` is whether the target is a file at the
    commit, not whether it is in this step's view."""

    __slots__ = ("index", "relationship", "target", "resolved", "exists")

    def __init__(self, index, relationship, target, resolved, exists):
        self.index = index
        self.relationship = relationship
        self.target = target
        self.resolved = resolved
        self.exists = exists

    def __repr__(self):
        return "Link(%r -> %r)" % (self.relationship, self.resolved)


class ClassProp:
    __slots__ = ("name", "type", "required")

    def __init__(self, name, type_, required):
        self.name = name
        self.type = type_
        self.required = required


class Class:
    __slots__ = ("name", "properties")

    def __init__(self, name):
        self.name = name
        self.properties = []


class Node:
    """One corpus node.

    `id` is repository-relative, the spelling `reads` uses. `ref` is the RFC-0032 reference a
    signal row is keyed on: `gage/canyon-outlet` for `.yidam/corpus/gage/canyon-outlet.yml`.
    `malformed` is the parser's reason when the file did not parse, and `None` when it did. A
    malformed node is still a node, and has no properties because none could be read.
    """

    __slots__ = ("id", "ref", "class_", "label", "malformed", "properties", "rows", "links")

    def __init__(self, id_, ref, class_, label):
        self.id = id_
        self.ref = ref
        self.class_ = class_
        self.label = label
        self.malformed = None
        self.properties = {}
        self.rows = []
        self.links = []

    def get(self, key, default=None):
        return self.properties.get(key, default)

    def __repr__(self):
        return "Node(%r)" % self.ref


class Graph:
    __slots__ = ("corpus_dir", "classes", "nodes", "_by")

    def __init__(self):
        self.corpus_dir = ""
        self.classes = []
        self.nodes = []
        self._by = {}

    def node(self, name):
        """A node by its id or its reference, or `None`."""
        return self._by.get(name)


def _unescape(field):
    if "\\" not in field:
        return field
    out = []
    i = 0
    while i < len(field):
        c = field[i]
        if c == "\\" and i + 1 < len(field):
            nxt = field[i + 1]
            mapped = {"\\": "\\", "t": "\t", "n": "\n", "r": "\r"}.get(nxt)
            if mapped is not None:
                out.append(mapped)
                i += 2
                continue
        out.append(c)
        i += 1
    return "".join(out)


_SPECIAL_FLOATS = {".inf": math.inf, "-.inf": -math.inf, ".nan": math.nan}


def _scalar(kind, raw, where):
    if kind == "null":
        return None
    if kind == "bool":
        if raw not in ("true", "false"):
            raise KitError("%s: a bool row holds %r" % (where, raw))
        return raw == "true"
    if kind == "int":
        try:
            return int(raw)
        except ValueError:
            raise KitError("%s: an int row holds %r" % (where, raw)) from None
    if kind == "float":
        if raw in _SPECIAL_FLOATS:
            return _SPECIAL_FLOATS[raw]
        try:
            return float(raw)
        except ValueError:
            raise KitError("%s: a float row holds %r" % (where, raw)) from None
    if kind == "string":
        return raw
    return Opaque(kind)


_ITEM = re.compile(r"\[([0-9]+)\](?:\.(.*))?\Z", re.S)


def _ref(corpus_dir, id_):
    prefix = corpus_dir + "/" if corpus_dir else ""
    ref = id_[len(prefix):] if prefix and id_.startswith(prefix) else id_
    return ref[:-4] if ref.endswith(".yml") else ref


def _add_prop(node, key, kind, raw):
    """Fold one `prop` row into the node's properties.

    A sequence row is followed by its items, `<key>[<i>]`, and an item that is a record by its
    fields, `<key>[<i>].<field>`. Rows arrive in that order or not at all.
    """
    where = "%s %s" % (node.id, key)
    if kind == "seq" and raw != "":
        # The row holds the item count; the property is the list the item rows fill.
        node.rows.append(Row(key, kind, _scalar("int", raw, where)))
        value = []
    else:
        value = Opaque(kind) if kind in ("seq", "map") else _scalar(kind, raw, where)
        node.rows.append(Row(key, kind, value))

    for name, held in reversed(list(node.properties.items())):
        if not isinstance(held, list) or not key.startswith(name + "["):
            continue
        m = _ITEM.match(key, len(name))
        if not m:
            break
        index, field = int(m.group(1)), m.group(2)
        if field is None:
            if index != len(held):
                raise KitError("%s: item %d arrived after %d items" % (where, index, len(held)))
            held.append({} if kind == "map" else value)
            return
        if index != len(held) - 1 or not isinstance(held[index], dict):
            raise KitError("%s: a field of an item that is not a record" % where)
        held[index][field] = value
        return
    node.properties[key] = value


def parse(text):
    """Read the graph from its text. See `read`."""
    graph = Graph()
    version = None
    for number, line in enumerate(text.split("\n"), 1):
        if not line or line.startswith("#"):
            continue
        fields = [_unescape(f) for f in line.split("\t")]
        kind = fields[0]
        if version is None and kind != "format_version":
            raise KitError("line %d: the graph must open with format_version" % number)
        if kind == "format_version":
            if fields[1:] != [str(GRAPH_FORMAT_VERSION)]:
                raise KitError(
                    "the graph is format_version %s and this kit reads %d. Update the kit"
                    % ("\t".join(fields[1:]), GRAPH_FORMAT_VERSION)
                )
            version = GRAPH_FORMAT_VERSION
        elif kind == "corpus_dir" and len(fields) >= 2:
            graph.corpus_dir = fields[1]
        elif kind == "class" and len(fields) >= 2:
            graph.classes.append(Class(fields[1]))
        elif kind == "class_prop" and len(fields) >= 5:
            for c in graph.classes:
                if c.name == fields[1]:
                    c.properties.append(ClassProp(fields[2], fields[3], fields[4] == "true"))
        elif kind == "node" and len(fields) >= 4:
            n = Node(fields[1], _ref(graph.corpus_dir, fields[1]), fields[2], fields[3])
            graph.nodes.append(n)
            graph._by[n.id] = n
            graph._by[n.ref] = n
        elif kind == "malformed" and len(fields) >= 3:
            n = graph._by.get(fields[1])
            if n is not None:
                n.malformed = fields[2]
        elif kind == "prop" and len(fields) >= 5:
            n = graph._by.get(fields[1])
            if n is not None:
                _add_prop(n, fields[2], fields[3], fields[4])
        elif kind == "link" and len(fields) >= 7:
            # Checked before the node is looked up, as the Rust kit does, and as ASCII digits:
            # `int` alone takes `" 1"`, `"1_0"` and other scripts' digits.
            if not re.fullmatch(r"[0-9]+", fields[2]):
                raise KitError("line %d: a link index is %r" % (number, fields[2]))
            n = graph._by.get(fields[1])
            if n is not None:
                n.links.append(
                    Link(int(fields[2]), fields[3], fields[4], fields[5], fields[6] == "true")
                )
        # Any other kind is one this kit has not heard of, and the format says to skip it.
    if version is None:
        raise KitError("the graph carries no format_version")
    return graph


def read(path=None):
    """Read the resolved corpus `yidam run` handed this step.

    `path` defaults to `$YIDAM_GRAPH`. A step whose `reads` admit no corpus node is handed no
    graph, and this says so rather than returning an empty one.
    """
    if path is None:
        path = os.environ.get("YIDAM_GRAPH")
        if not path:
            raise KitError(
                "no resolved corpus: $YIDAM_GRAPH is unset. Does this step's `reads` cover "
                ".yidam/corpus?"
            )
    with open(path, encoding="utf-8") as f:
        return parse(f.read())


# -- tables ----------------------------------------------------------------------------------


def _table_lines(text, fmt):
    """The rows of a table file, the header first, read as `yidam`'s own reader reads them."""
    # A leading byte-order mark is not part of the first cell, to `yidam` either.
    if text.startswith("\ufeff"):
        text = text[1:]
    if fmt == "csv":
        # `yidam`'s reader has no cap on a cell, and Python's is 128 KiB unless raised.
        if csv.field_size_limit() <= len(text):
            csv.field_size_limit(min(len(text) + 1, 2**31 - 1))
        # A blank line is no record, as it is to `yidam`'s reader.
        try:
            return [r for r in csv.reader(io.StringIO(text, newline="")) if r]
        except csv.Error as e:
            raise KitError("not readable as csv: %s" % e) from None
    rows = text.split("\n")
    if text.endswith("\n"):
        rows.pop()
    return [(r[:-1] if r.endswith("\r") else r).split("\t") for r in rows]


def _table_number(v):
    # `yidam`'s rule: a finite decimal, as Rust reads one. Python's float() also reads `1_000`
    # and non-ASCII digits, which `yidam lint` would report, so neither is a number here.
    if not v.isascii() or "_" in v:
        return None
    try:
        x = float(v)
    except ValueError:
        return None
    return x if math.isfinite(x) else None


def parse_table(declarations, path, text):
    """Read one table's text by its declaration in `declarations`, the `$YIDAM_TABLES` document.

    A list of rows, each a dict in the declaration's column order. A `number` cell is a float,
    an empty cell is `None`, and every other cell is its text, trimmed. A path no declaration
    names is refused, so an undeclared file is not read as if it were typed.
    """
    decl = next((d for d in json.loads(declarations) if d["path"] == path), None)
    if decl is None:
        raise KitError(
            "%s: no table declaration in $YIDAM_TABLES names it. Does a catalog entry declare "
            "its `columns:`, and does this step's `reads` cover the file?" % path
        )
    columns = decl["columns"]
    names = [c["name"] for c in columns]
    lines = _table_lines(text, decl["format"])
    header = [h.strip() for h in lines[0]] if lines else []
    if header != names:
        raise KitError(
            "%s: the header reads `%s`, and `%s` declares `%s`"
            % (path, " ".join(header), decl["entry"], " ".join(names))
        )
    rows = []
    for number, cells in enumerate(lines[1:], 2):
        if len(cells) != len(columns):
            raise KitError(
                "%s: row %d has %d cells, and `%s` declares %d columns"
                % (path, number, len(cells), decl["entry"], len(columns))
            )
        row = {}
        for col, cell in zip(columns, cells):
            v = cell.strip()
            if v == "":
                row[col["name"]] = None
            elif col["type"] == "number":
                row[col["name"]] = _table_number(v)
                if row[col["name"]] is None:
                    raise KitError(
                        "%s: row %d, column `%s`: `%s` is not a number"
                        % (path, number, col["name"], v)
                    )
            else:
                row[col["name"]] = v
        rows.append(row)
    return rows


def table(path, tables=None, root=None):
    """Read the declared table at `path`, repository-relative as its entry's `file` location.

    `tables` defaults to `$YIDAM_TABLES` and `root` to `$YIDAM_IN`, which `yidam run` sets. See
    `parse_table` for what a row is.
    """
    if tables is None:
        tables = os.environ.get("YIDAM_TABLES")
        if not tables:
            raise KitError(
                "no table declarations: $YIDAM_TABLES is unset. Does this step's `reads` cover "
                "a table a catalog entry declares?"
            )
    if root is None:
        root = os.environ.get("YIDAM_IN", ".")
    with open(tables, encoding="utf-8") as f:
        declarations = f.read()
    with open(os.path.join(root, path), encoding="utf-8", newline="") as f:
        return parse_table(declarations, path, f.read())


# -- writing ---------------------------------------------------------------------------------

_BARE_KEY = re.compile(r"[A-Za-z_][A-Za-z0-9_-]*\Z")
_WORDS = frozenset(["true", "false", "null", "yes", "no", "on", "off", "y", "n"])

# Escaped as `\uXXXX` although they are not controls: no YAML reader accepts U+FFFE or U+FFFF
# raw, and YAML 1.1 reads U+2028 and U+2029 as line breaks, which fold to a space inside a
# quoted string.
_UNICODE_ESCAPED = frozenset([" ", " ", "￾", "￿"])

_INT_MIN = -(2**63)
_INT_MAX = 2**64 - 1


def _quote(s):
    out = ['"']
    for c in s:
        o = ord(c)
        if c == '"':
            out.append('\\"')
        elif c == "\\":
            out.append("\\\\")
        elif o < 0x20 or 0x7F <= o <= 0x9F:
            out.append("\\x%02x" % o)
        elif c in _UNICODE_ESCAPED:
            out.append("\\u%04x" % o)
        elif 0xD800 <= o <= 0xDFFF:
            raise KitError("a string holds a lone surrogate, U+%04X, which is not text" % o)
        else:
            out.append(c)
    out.append('"')
    return "".join(out)


def _float(x):
    if not math.isfinite(x):
        raise KitError(
            "%r is not a finite float. JSON cannot carry it; write a string if you mean an "
            "unbounded value" % x
        )
    r = repr(x)
    mantissa, e, exponent = r.partition("e")
    if "." not in mantissa:
        mantissa += ".0"
    return mantissa + e + exponent


def _key(k):
    if not isinstance(k, str):
        raise KitError("a mapping key must be a string, not %r" % (k,))
    if _BARE_KEY.match(k) and k.lower() not in _WORDS:
        return k
    return _quote(k)


def _scalar_text(v):
    if v is None:
        return "null"
    if v is True:
        return "true"
    if v is False:
        return "false"
    if isinstance(v, int):
        if not _INT_MIN <= v <= _INT_MAX:
            raise KitError("%d does not fit in 64 bits" % v)
        return str(v)
    if isinstance(v, float):
        return _float(v)
    if isinstance(v, str):
        return _quote(v)
    if isinstance(v, dict) and not v:
        return "{}"
    if isinstance(v, (list, tuple)) and not v:
        return "[]"
    if isinstance(v, (dict, list, tuple)):
        return None
    raise KitError("cannot write a %s: %r" % (type(v).__name__, v))


def _block(v, indent, out):
    pad = " " * indent
    if isinstance(v, dict):
        for k, x in v.items():
            s = _scalar_text(x)
            if s is not None:
                out.append("%s%s: %s" % (pad, _key(k), s))
            else:
                out.append("%s%s:" % (pad, _key(k)))
                _block(x, indent + 2, out)
    else:
        for x in v:
            s = _scalar_text(x)
            if s is not None:
                out.append("%s- %s" % (pad, s))
            else:
                start = len(out)
                _block(x, indent + 2, out)
                out[start] = pad + "- " + out[start][indent + 2:]


def dump(value):
    """Block YAML for a tree of dicts, lists and scalars, ending in a newline.

    Every string is double-quoted, so a string reads back as the string it was in every YAML
    reader: `00:00`, `yes`, `a: b` and `#x` included. A float always carries a `.`. A key is
    bare only when no YAML schema reads it as anything but a string.
    """
    s = _scalar_text(value)
    if s is not None:
        return s + "\n"
    out = []
    _block(value, 0, out)
    return "\n".join(out) + "\n"


def signals(rows, method=None, **fields):
    """A signal table: `format_version`, then `method` when given, `signals`, then `fields`.

    Every row is a mapping with a string `node:`, the RFC-0032 reference of the node it is
    about (a `Node`'s `ref`). The other keys of a row are the signals.
    """
    rows = list(rows)
    for i, row in enumerate(rows):
        if not isinstance(row, dict) or not isinstance(row.get("node"), str):
            raise KitError("signal row %d has no `node:` naming the node it is about" % i)
    table = {"format_version": SIGNALS_FORMAT_VERSION}
    if method is not None:
        table["method"] = method
    table["signals"] = rows
    for k, v in fields.items():
        if k in table:
            raise KitError("`%s` is written by signals() itself" % k)
        table[k] = v
    return dump(table)
