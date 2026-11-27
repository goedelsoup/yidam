//! The calculator kit, held to `yidam`'s own reader and writer (RFC-0056 §2.5).
//!
//! `yidam/prelude/kit/` holds the kit in two languages. Neither is compiled into `yidam`, so
//! nothing but these tests says either one reads the graph `resolved::build` writes, or writes
//! YAML `computed::Signals` reads back. Each test asks one of those questions against `yidam`'s
//! own code, and asks the Python kit the same question by running it.
//!
//! Lib unit tests and not suite tests, because the renderer and the corpus reader are private
//! to the crate, and the question is whether the kit agrees with *them*.

#[allow(dead_code)]
#[path = "../../../../prelude/kit/yidam_kit.rs"]
mod yidam_kit;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value as Json};
use yidam_kit::Value;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn kit_dir() -> PathBuf {
    repo().join("yidam/prelude/kit")
}

/// The graph `yidam run` would hand a step reading every node at `root`.
fn graph_of(root: &Path) -> String {
    let read = crate::corpus::Corpus::open(root);
    super::resolved::build(&read, &|_| true, &|p| root.join(p).is_file()).expect("a graph")
}

fn write(root: &Path, rel: &str, text: &str) {
    let at = root.join(rel);
    std::fs::create_dir_all(at.parent().expect("a parent")).expect("mkdir");
    std::fs::write(at, text).expect("write");
}

/// A corpus holding every property kind the graph carries, every escape, a list of records
/// with fields of every depth, a malformed node, and a link of each resolution.
fn every_kind() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    write(
        root,
        ".yidam/corpus/claim/a.yml",
        "class: claim\n\
         label: \"tab\\there, newline\\n, back \\\\ and return\\r\"\n\
         properties:\n\
         \x20 s: \"x\\ty\"\n\
         \x20 quoted_number: \"1\"\n\
         \x20 u: 18446744073709551615\n\
         \x20 n: -9223372036854775808\n\
         \x20 f: 1.5e-05\n\
         \x20 big: 1.0e+300\n\
         \x20 whole: 12.0\n\
         \x20 inf: .inf\n\
         \x20 ninf: -.inf\n\
         \x20 b: true\n\
         \x20 z: null\n\
         \x20 empty: []\n\
         \x20 tags: [x, \"y\\tz\", 3]\n\
         \x20 recs:\n\
         \x20   - {name: one, w: 1.5, nested: {a: 1}, inner: [1], none: null}\n\
         \x20   - plain\n\
         \x20   - [1, 2]\n\
         \x20   - {}\n\
         \x20 m: {a: 1}\n\
         \x20 t: !tagged x\n\
         links:\n\
         \x20 - target: ../claim/b.yml\n\
         \x20   relationship: supports\n\
         \x20 - target: ../nowhere/c.yml\n\
         \x20   relationship: cites\n\
         \x20 - relationship: empty\n",
    );
    write(
        root,
        ".yidam/corpus/claim/b.yml",
        "class: claim\nlabel: B\n",
    );
    write(
        root,
        ".yidam/corpus/claim/broken.yml",
        "class: claim\nproperties: [unclosed\n",
    );
    dir
}

/// A record kind this kit has never heard of, which the format says a reader skips.
fn with_unknown_kind(graph: &str) -> String {
    format!("{graph}future_kind\t.yidam/corpus/claim/a.yml\tsomething\n")
}

// ── the kit against yidam's reader ──────────────────────────────────────────────────────

/// A property as the graph carries it, from yidam's own parse of the node file.
fn expected(v: &serde_yaml::Value, depth: usize) -> Value {
    use serde_yaml::Value as Y;
    match v {
        Y::Null => Value::Null,
        Y::Bool(b) => Value::Bool(*b),
        Y::Number(n) if n.is_i64() || n.is_u64() => {
            Value::Int(n.to_string().parse().expect("an int"))
        }
        Y::Number(n) => Value::Float(n.as_f64().expect("a float")),
        Y::String(s) => Value::Str(s.clone()),
        // A top-level list, and the items of it; nothing deeper has a value.
        Y::Sequence(items) if depth == 0 => {
            Value::List(items.iter().map(|i| expected(i, 1)).collect())
        }
        // An item that is a record, and its fields; a field that is a mapping has none.
        Y::Mapping(m) if depth == 1 => Value::Map(
            m.iter()
                .filter_map(|(k, x)| Some((k.as_str()?.to_string(), expected(x, 2))))
                .collect(),
        ),
        Y::Sequence(_) => Value::Opaque("seq".into()),
        Y::Mapping(_) => Value::Opaque("map".into()),
        Y::Tagged(_) => Value::Opaque("tagged".into()),
    }
}

/// Every node the kit read is one yidam read, with the same class, label, malformed state,
/// properties and links, in the same order.
fn assert_reads_as_yidam_does(root: &Path, graph: &yidam_kit::Graph) {
    let read = crate::corpus::Corpus::open(root);
    let ids: Vec<&str> = graph.nodes.iter().map(|n| n.id.as_str()).collect();
    let want: Vec<&str> = read.nodes().iter().map(|n| n.rel.as_str()).collect();
    assert_eq!(ids, want, "the kit's nodes are yidam's, in yidam's order");
    for (k, n) in graph.nodes.iter().zip(read.nodes()) {
        assert_eq!(
            k.class,
            n.inst.class.clone().unwrap_or_default(),
            "{}",
            n.rel
        );
        assert_eq!(
            k.label,
            n.inst.label.clone().unwrap_or_default(),
            "{}",
            n.rel
        );
        assert_eq!(k.malformed, n.malformed, "{}", n.rel);
        let props: Vec<(String, Value)> = n
            .inst
            .properties
            .iter()
            .flatten()
            .filter_map(|(key, v)| Some((key.as_str()?.to_string(), expected(v, 0))))
            .collect();
        assert_eq!(k.properties, props, "{}", n.rel);
        let links: Vec<(String, String)> = n
            .inst
            .links
            .iter()
            .flatten()
            .map(|l| {
                (
                    l.relationship.clone().unwrap_or_default(),
                    l.target.clone().unwrap_or_default(),
                )
            })
            .collect();
        let got: Vec<(String, String)> = k
            .links
            .iter()
            .map(|l| (l.relationship.clone(), l.target.clone()))
            .collect();
        assert_eq!(got, links, "{}", n.rel);
    }
    let classes: Vec<&str> = graph.classes.iter().map(|c| c.name.as_str()).collect();
    let want: Vec<&str> = read.classes().iter().map(|c| c.name.as_str()).collect();
    assert_eq!(classes, want);
}

fn streamflow() -> PathBuf {
    repo().join("examples/streamflow")
}

#[test]
fn the_rust_kit_reads_the_example_corpus_as_yidam_does() {
    let root = streamflow();
    let graph = yidam_kit::parse(&graph_of(&root)).expect("parses");
    assert!(graph.nodes.len() >= 8, "{} nodes", graph.nodes.len());
    assert!(
        graph.classes.iter().any(|c| !c.properties.is_empty()),
        "a class with properties"
    );
    assert_eq!(
        graph.node("gage/canyon-outlet").map(|n| n.id.as_str()),
        Some(".yidam/corpus/gage/canyon-outlet.yml"),
        "a node by its reference"
    );
    assert_reads_as_yidam_does(&root, &graph);
}

#[test]
fn the_rust_kit_reads_every_kind_and_escape_as_yidam_does() {
    let dir = every_kind();
    let text = with_unknown_kind(&graph_of(dir.path()));
    let graph = yidam_kit::parse(&text).expect("parses");
    assert_reads_as_yidam_does(dir.path(), &graph);

    let a = graph.node("claim/a").expect("a");
    assert_eq!(a.label, "tab\there, newline\n, back \\ and return\r");
    assert_eq!(a.get("u"), Some(&Value::Int(18446744073709551615)));
    assert_eq!(a.get("quoted_number"), Some(&Value::Str("1".into())));
    assert_eq!(a.get("inf"), Some(&Value::Float(f64::INFINITY)));
    let exists: Vec<(&str, bool)> = a
        .links
        .iter()
        .map(|l| (l.resolved.as_str(), l.exists))
        .collect();
    assert_eq!(
        exists,
        [
            (".yidam/corpus/claim/b.yml", true),
            (".yidam/corpus/nowhere/c.yml", false),
            ("", false)
        ]
    );
    assert!(graph
        .node("claim/broken")
        .expect("broken")
        .malformed
        .is_some());
}

#[test]
fn the_rust_kit_refuses_a_graph_it_cannot_read() {
    let e = yidam_kit::parse("format_version\t2\nnode\ta\tb\tc\n").expect_err("v2");
    assert!(e.0.contains("Update the kit"), "{e}");
    let e = yidam_kit::parse("node\ta\tb\tc\n").expect_err("no version");
    assert!(e.0.contains("format_version"), "{e}");
}

// ── the Python kit, asked the same questions ────────────────────────────────────────────

/// Python, in isolated mode, with the kit's directory as an argument rather than on a path the
/// environment could reach. The CI image provides `python3` through mise.
fn python(script: &str, args: &[&Path]) -> String {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("probe.py");
    std::fs::write(&path, script).expect("write the probe");
    let out = Command::new("python3")
        .arg("-I")
        .arg(&path)
        .arg(kit_dir())
        .args(args)
        .env("PYTHONPATH", "")
        .output()
        .expect("python3 runs");
    assert!(
        out.status.success(),
        "python3 failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("utf-8")
}

/// Both kits' readings in one encoding. A float is its kit's own written form, so a reading
/// that differs in the last bit differs here too.
const PY_READ: &str = r#"
import json, math, sys
sys.path.insert(0, sys.argv[1])
import yidam_kit as k

def enc(v):
    if isinstance(v, k.Opaque):
        return {"opaque": v.kind}
    if v is None or isinstance(v, (bool, str)):
        return v
    if isinstance(v, int):
        return {"int": str(v)}
    if isinstance(v, float):
        if math.isnan(v): return {"float": "nan"}
        if math.isinf(v): return {"float": "inf" if v > 0 else "-inf"}
        return {"float": k.dump(v).strip()}
    if isinstance(v, list):
        return [enc(x) for x in v]
    return [[a, enc(b)] for a, b in v.items()]

with open(sys.argv[2], encoding="utf-8") as f:
    g = k.parse(f.read())
print(json.dumps({
    "corpus_dir": g.corpus_dir,
    "classes": [[c.name, [[p.name, p.type, p.required] for p in c.properties]] for c in g.classes],
    "nodes": [{
        "id": n.id, "ref": n.ref, "class": n.class_, "label": n.label, "malformed": n.malformed,
        "properties": [[a, enc(b)] for a, b in n.properties.items()],
        "rows": [[r.key, r.kind, enc(r.value)] for r in n.rows],
        "links": [[l.index, l.relationship, l.target, l.resolved, l.exists] for l in n.links],
    } for n in g.nodes],
}, sort_keys=True))
"#;

fn enc(v: &Value) -> Json {
    match v {
        Value::Opaque(kind) => json!({ "opaque": kind }),
        Value::Null => Json::Null,
        Value::Bool(b) => json!(b),
        Value::Str(s) => json!(s),
        Value::Int(i) => json!({ "int": i.to_string() }),
        Value::Float(x) if x.is_nan() => json!({ "float": "nan" }),
        Value::Float(x) if x.is_infinite() => {
            json!({ "float": if *x > 0.0 { "inf" } else { "-inf" } })
        }
        Value::Float(_) => json!({ "float": yidam_kit::dump(v).expect("finite").trim() }),
        Value::List(items) => Json::Array(items.iter().map(enc).collect()),
        Value::Map(m) => Json::Array(m.iter().map(|(a, b)| json!([a, enc(b)])).collect()),
    }
}

fn encode(g: &yidam_kit::Graph) -> Json {
    json!({
        "corpus_dir": g.corpus_dir,
        "classes": g.classes.iter().map(|c| json!([
            c.name,
            c.properties.iter().map(|p| json!([p.name, p.r#type, p.required])).collect::<Vec<_>>()
        ])).collect::<Vec<_>>(),
        "nodes": g.nodes.iter().map(|n| json!({
            "id": n.id, "ref": n.reference, "class": n.class, "label": n.label,
            "malformed": n.malformed,
            "properties": n.properties.iter().map(|(a, b)| json!([a, enc(b)])).collect::<Vec<_>>(),
            "rows": n.rows.iter().map(|r| json!([r.key, r.kind, enc(&r.value)])).collect::<Vec<_>>(),
            "links": n.links.iter().map(|l| json!([
                l.index, l.relationship, l.target, l.resolved, l.exists
            ])).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

fn assert_kits_agree(text: &str) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("corpus.tsv");
    std::fs::write(&path, text).expect("write the graph");
    let theirs: Json = serde_json::from_str(&python(PY_READ, &[&path])).expect("json");
    let ours = encode(&yidam_kit::parse(text).expect("parses"));
    assert_eq!(
        theirs, ours,
        "the Python kit reads the graph as the Rust kit does"
    );
}

#[test]
fn the_python_kit_reads_the_example_corpus_as_the_rust_kit_does() {
    assert_kits_agree(&graph_of(&streamflow()));
}

#[test]
fn the_python_kit_reads_every_kind_and_escape_as_the_rust_kit_does() {
    let dir = every_kind();
    assert_kits_agree(&with_unknown_kind(&graph_of(dir.path())));
}

// ── the writers, against each other and against yidam's readers ────────────────────────

fn hostile() -> Json {
    let path = repo().join("yidam/cli/tests/fixtures/calculator-kit/hostile.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("the fixture")).expect("json")
}

/// A JSON value as the kit's, keys in byte order: `serde_json`'s map is sorted, and the Python
/// probe sorts to match.
fn from_json(v: &Json) -> Value {
    match v {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) if n.is_i64() || n.is_u64() => {
            Value::Int(n.to_string().parse().expect("an int"))
        }
        Json::Number(n) => Value::Float(n.as_f64().expect("a float")),
        Json::String(s) => Value::Str(s.clone()),
        Json::Array(a) => Value::List(a.iter().map(from_json).collect()),
        Json::Object(m) => Value::Map(m.iter().map(|(k, x)| (k.clone(), from_json(x))).collect()),
    }
}

const PY_WRITE: &str = r#"
import json, sys
sys.path.insert(0, sys.argv[1])
import yidam_kit as k

with open(sys.argv[2], encoding="utf-8") as f:
    d = json.load(f, object_pairs_hook=lambda pairs: dict(sorted(pairs)))
out = [k.dump(v) for v in d["values"]] + [k.dump(d)]
out.append(k.signals(d["rows"], method={"rule": "hostile"}, summary={"rows": len(d["rows"])}))
print(json.dumps(out))
"#;

fn rust_writes(d: &Json) -> Vec<String> {
    let values = d["values"].as_array().expect("values");
    let mut out: Vec<String> = values
        .iter()
        .map(|v| yidam_kit::dump(&from_json(v)).expect("dumps"))
        .collect();
    out.push(yidam_kit::dump(&from_json(d)).expect("dumps"));
    let rows = d["rows"].as_array().expect("rows");
    out.push(
        yidam_kit::signals(
            rows.iter().map(from_json).collect(),
            Some(Value::map([("rule", "hostile".into())])),
            vec![("summary".into(), Value::map([("rows", Value::Int(1))]))],
        )
        .expect("signals"),
    );
    out
}

#[test]
fn the_two_writers_write_the_same_bytes() {
    let d = hostile();
    let path = repo().join("yidam/cli/tests/fixtures/calculator-kit/hostile.json");
    let theirs: Vec<String> = serde_json::from_str(&python(PY_WRITE, &[&path])).expect("json");
    let ours = rust_writes(&d);
    assert_eq!(theirs.len(), ours.len());
    for (t, o) in theirs.iter().zip(&ours) {
        assert_eq!(t, o, "the Python kit and the Rust kit wrote different YAML");
    }
}

#[test]
fn what_the_kit_writes_reads_back_in_yidam_as_what_it_was_given() {
    let d = hostile();
    let values = d["values"].as_array().expect("values");
    let written = rust_writes(&d);
    for (v, text) in values.iter().chain([&d]).zip(&written) {
        let back: Json = serde_yaml::from_str(text).unwrap_or_else(|e| panic!("{e}\n{text}"));
        assert_eq!(&back, v, "read back differently:\n{text}");
    }
}

#[test]
fn a_signal_table_the_kit_writes_is_one_signals_reads_without_a_problem() {
    let d = hostile();
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    write(
        root,
        ".yidam/corpus/gage/canyon-outlet.yml",
        "label: Canyon Outlet\n",
    );
    write(
        root,
        super::manifest::MANIFEST,
        "[capability.kit]\nkind = \"calculator\"\nrun = [\"true\"]\n\
         reads = [\".yidam/corpus/**\"]\nwrites = [\".yidam/computed/**\"]\nverb = \"compute\"\n",
    );
    let table = rust_writes(&d).pop().expect("the table");
    write(root, ".yidam/computed/kit.yml", &table);

    let s = crate::computed::Signals::load(root);
    assert!(s.problems.is_empty(), "{:?}\n{table}", s.problems);
    let mut want = d["rows"][0].as_object().expect("a row").clone();
    want.remove("node");
    let got = s.for_node(".yidam/corpus/gage/canyon-outlet.yml");
    assert_eq!(
        got.into_iter().collect::<serde_json::Map<_, _>>(),
        want,
        "{table}"
    );
}

#[test]
fn the_writers_refuse_what_no_reader_would_read_back() {
    for bad in [
        Value::Float(f64::NAN),
        Value::Float(f64::INFINITY),
        Value::Int(i128::from(u64::MAX) + 1),
        Value::Int(i128::from(i64::MIN) - 1),
        Value::Opaque("map".into()),
    ] {
        assert!(yidam_kit::dump(&bad).is_err(), "{bad:?} was written");
    }
    let no_node = yidam_kit::signals(vec![Value::map([("x", Value::Int(1))])], None, vec![]);
    assert!(no_node.is_err());
    let doubled = yidam_kit::signals(vec![], None, vec![("signals".into(), Value::Null)]);
    assert!(doubled.is_err());
}

// ── the kit as a calculator receives it ─────────────────────────────────────────────────

/// `yidam_kit.rs` is included by path into a calculator crate, so it must compile with `std`
/// and nothing else. Being a module of this crate would not show that: a `use serde_json`
/// would compile here and nowhere else.
#[test]
fn the_rust_kit_compiles_alone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let out = Command::new(rustc)
        .args(["--edition", "2021", "--crate-type", "lib", "-D", "warnings"])
        .arg("--out-dir")
        .arg(dir.path())
        .arg(kit_dir().join("yidam_kit.rs"))
        .output()
        .expect("rustc runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The example has no vendored prelude, so its step reads a copy of the kit beside it. The copy
/// is the kit, or the example shows a calculator using a kit no repository has.
#[test]
fn the_example_s_copy_of_the_kit_is_the_kit() {
    let copy = streamflow().join(".yidam/capabilities/yidam_kit.py");
    assert_eq!(
        std::fs::read(&copy).expect("the example's copy"),
        std::fs::read(kit_dir().join("yidam_kit.py")).expect("the kit"),
        "{} differs from yidam/prelude/kit/yidam_kit.py. Copy the kit over it",
        copy.display()
    );
}

// ── declared tables (RFC-0059 §2.4) ─────────────────────────────────────────────────────

fn column(name: &str, r#type: &str) -> crate::table::Column {
    crate::table::Column {
        name: name.into(),
        r#type: r#type.into(),
        required: false,
        unit: None,
        values: None,
    }
}

/// One declaration of `id`, `amount` (a number) and `note`, at `path`, as `yidam run` writes it.
fn tables_document(path: &str) -> (crate::table::Declaration, String) {
    let decl = crate::table::Declaration {
        entry: "receipts".into(),
        entry_rel: ".yidam/catalog/receipts.md".into(),
        path: path.into(),
        format: crate::table::Format::of(path).expect("a table path"),
        columns: vec![
            column("id", "string"),
            column("amount", "number"),
            column("note", "string"),
        ],
        key: vec!["id".into()],
    };
    let doc = crate::table::document(std::slice::from_ref(&decl), &|_| true).expect("covered");
    (decl, doc)
}

const PY_TABLE: &str = r#"
import json, math, sys
sys.path.insert(0, sys.argv[1])
import yidam_kit as k

def enc(v):
    if v is None or isinstance(v, str):
        return v
    return {"float": k.dump(v).strip()}

with open(sys.argv[2], encoding="utf-8") as f:
    doc = f.read()
with open(sys.argv[4], encoding="utf-8", newline="") as f:
    text = f.read()
try:
    rows = k.parse_table(doc, sys.argv[3], text)
    print(json.dumps([[[a, enc(b)] for a, b in r.items()] for r in rows]))
except k.KitError as e:
    print(json.dumps({"error": str(e)}))
"#;

/// The Rust kit's reading of `text` at `path`, in the Python probe's encoding.
fn rust_table(doc: &str, path: &str, text: &str) -> Json {
    match yidam_kit::parse_table(doc, path, text) {
        Ok(rows) => Json::Array(rows.iter().map(enc).collect()),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// The Python kit's reading of `text` at `path`.
fn python_table(doc: &str, path: &str, text: &str) -> Json {
    let dir = tempfile::tempdir().expect("tempdir");
    let (d, t) = (dir.path().join("tables.json"), dir.path().join("table"));
    std::fs::write(&d, doc).expect("write the document");
    std::fs::write(&t, text).expect("write the table");
    let out = python(PY_TABLE, &[&d, Path::new(path), &t]);
    serde_json::from_str(&out).expect("the probe prints JSON")
}

/// Both kits read a table as `yidam` holds it: the file `lint` finds nothing wrong with reads,
/// in both, as the same typed rows.
#[test]
fn both_kits_read_a_declared_table_alike_and_as_yidam_holds_it() {
    let cases = [
        (
            "data/receipts.tsv",
            "id\tamount\tnote\nA\t12.50\t paid \nB\t\t\nC\t-3e2\tlate\n",
        ),
        (
            "data/receipts.csv",
            "id,amount,note\r\nA,12.50,\"paid, in full\"\r\n\r\nB,,\"two\nlines\"\r\nC,-3e2,\"a \"\"quoted\"\" word\"",
        ),
    ];
    for (path, text) in cases {
        let (decl, doc) = tables_document(path);
        let held = crate::table::read(text, decl.format).expect("yidam reads it");
        assert!(crate::table::shape(&decl, &held).is_empty(), "{path}");
        let rust = rust_table(&doc, path, text);
        assert_eq!(rust, python_table(&doc, path, text), "{path}");
        let rows = rust.as_array().expect("rows");
        assert_eq!(rows.len(), held.rows.len(), "{path}");
        assert_eq!(rows[0][1], json!(["amount", { "float": "12.5" }]), "{path}");
        assert_eq!(
            rows[1][1],
            json!(["amount", null]),
            "{path}: an empty cell is null"
        );
        assert_eq!(
            rows[2][1],
            json!(["amount", { "float": "-300.0" }]),
            "{path}"
        );
    }
    let (_, doc) = tables_document("data/receipts.csv");
    let text = "id,amount,note\nA,1,\"paid, in full\"\nB,2,\"two\nlines\"\n";
    let rows = rust_table(&doc, "data/receipts.csv", text);
    assert_eq!(rows[0][2], json!(["note", "paid, in full"]));
    assert_eq!(rows[1][2], json!(["note", "two\nlines"]));
}

/// A file that starts with a byte-order mark, as a spreadsheet saves one, and a CSV cell larger
/// than Python's default field limit read in both kits as they do in `yidam`.
#[test]
fn both_kits_read_a_byte_order_mark_and_a_large_cell_as_yidam_does() {
    let big = "x".repeat(200 * 1024);
    let cases = [
        (
            "data/receipts.tsv",
            "\u{feff}id\tamount\tnote\nA\t1\tx\n".to_string(),
        ),
        (
            "data/receipts.csv",
            "\u{feff}id,amount,note\r\nA,1,x\r\n".to_string(),
        ),
        (
            "data/receipts.csv",
            format!("id,amount,note\nA,1,\"{big}\"\n"),
        ),
    ];
    for (path, text) in &cases {
        let (decl, doc) = tables_document(path);
        let held = crate::table::read(text, decl.format).expect("yidam reads it");
        assert!(crate::table::shape(&decl, &held).is_empty(), "{path}");
        let rust = rust_table(&doc, path, text);
        assert_eq!(rust, python_table(&doc, path, text), "{path}");
        assert_eq!(rust[0][0], json!(["id", "A"]), "{path}");
    }
}

/// A file no declaration names, a header the declaration does not describe, a short row and a
/// cell that is not a number are refused by both kits, in the same words.
#[test]
fn both_kits_refuse_what_the_declaration_does_not_describe() {
    let (_, doc) = tables_document("data/receipts.tsv");
    let cases = [
        (
            "data/other.tsv",
            "id\tamount\tnote\n",
            "no table declaration",
        ),
        (
            "data/receipts.tsv",
            "id\tamt\tnote\n",
            "the header reads `id amt note`",
        ),
        (
            "data/receipts.tsv",
            "id\tamount\tnote\nA\t1\n",
            "row 2 has 2 cells",
        ),
        (
            "data/receipts.tsv",
            "id\tamount\tnote\nA\tten\tx\n",
            "is not a number",
        ),
        (
            "data/receipts.tsv",
            "id\tamount\tnote\nA\t1_000\tx\n",
            "is not a number",
        ),
        (
            "data/receipts.tsv",
            "id\tamount\tnote\nA\tinf\tx\n",
            "is not a number",
        ),
    ];
    for (path, text, said) in cases {
        let rust = rust_table(&doc, path, text);
        assert_eq!(rust, python_table(&doc, path, text), "{text:?}");
        let error = rust["error"]
            .as_str()
            .unwrap_or_else(|| panic!("{text:?} is refused"));
        assert!(error.contains(said), "{error}");
    }
}

/// `table()` outside a step that reads a declared table says why, rather than reading the file
/// untyped. The Rust kit is handed the unset variable rather than having it removed from this
/// process, whose other test threads read the environment (#1644).
#[test]
fn both_kits_refuse_a_table_read_with_no_declarations() {
    let rust = yidam_kit::table_from(None, None, "data/receipts.tsv").expect_err("no document");
    assert!(
        rust.to_string().contains("$YIDAM_TABLES is unset"),
        "{rust}"
    );
    let probe = r#"
import os, sys
sys.path.insert(0, sys.argv[1])
import yidam_kit as k
os.environ.pop("YIDAM_TABLES", None)
try:
    k.table("data/receipts.tsv")
except k.KitError as e:
    print(e)
"#;
    assert!(python(probe, &[]).contains("$YIDAM_TABLES is unset"));
}
