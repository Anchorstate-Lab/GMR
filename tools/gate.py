#!/usr/bin/env python3
"""Topology and discipline invariants gate.sh's cargo steps cannot express.

Invoked by gate.sh after cargo fmt/clippy/test. Exits 0 if every check below
holds, otherwise prints every violation found (not just the first) and exits 1.
"""

import hashlib
import json
import pathlib
import re
import subprocess
import sys

try:
    import tomllib
except ImportError:
    try:
        import tomli as tomllib
    except ImportError:
        print(
            "gate.py: need tomllib (Python 3.11+) or tomli (pip install tomli)",
            file=sys.stderr,
        )
        sys.exit(1)

ROOT = pathlib.Path(__file__).resolve().parent.parent
ARCH_TOML = ROOT / "architecture.toml"
FACADE = ROOT / "crates" / "gmr" / "src" / "lib.rs"
ACCEPTANCE = ROOT / "acceptance.sh"
WORKFLOW = ROOT / ".github" / "workflows" / "rust.yml"
CONTRACT_MODULE = ROOT / "crates" / "gmr-runtime" / "src" / "contract.rs"

PURE_ROOTS = ["gmr-budget", "gmr-core", "gmr-expr"]

NO_CONCRETE_IMPL = {
    "gmr-probe": {"tokio", "reqwest", "hyper"},
    "gmr-content": {"tokio", "reqwest", "hyper"},
    "gmr-store": {"sqlx", "rusqlite", "libsqlite3", "postgres", "tokio-postgres"},
}

DIR_LAYERS = {"crates": 0, "batteries": 1, "packs": 2, "console": 2}

CLEAN_ZONES = [
    "crates/gmr-core",
    "crates/gmr-content",
    "crates/gmr",
    "crates/gmr-probe",
    "crates/gmr-expr",
    "crates/gmr-store",
    "crates/gmr-runtime",
    "batteries/survey",
    "batteries/atlas",
    "batteries/provider",
    "batteries/transport",
    "packs/coding/extract",
    "console/cli",
    "console/core",
    "console/node",
    "console/python",
]
EXEMPT_FILES = ["console/cli/src/cli.rs"]


def run(cmd, **kwargs):
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, **kwargs)


def load_forbidden_libs():
    with open(ARCH_TOML, "rb") as f:
        data = tomllib.load(f)
    libs = data.get("libs", {})
    layers = data.get("layer", {})
    members = []
    for m in data.get("member", []):
        if m.get("kind") != "package":
            continue
        keys = (
            layers.get(m["layer"], {}).get("forbidden", [])
            + m.get("forbidden", [])
            + m.get("forbidden_default", [])
        )
        banned = {name for key in keys for name in libs.get(key, [])}
        members.append((m["name"], ROOT / m["path"], banned))
    return members


def check_pure_roots():
    errors = []
    for crate in PURE_ROOTS:
        r = run(["cargo", "tree", "-p", crate, "--edges", "normal", "--prefix", "none"])
        if r.returncode:
            errors.append(f"cannot read the dependency tree of {crate}: {r.stderr.strip()}")
            continue
        deps = [line.split()[0] for line in r.stdout.splitlines()[1:] if line.strip()]
        workspace_deps = [d for d in deps if d.startswith("gmr-")]
        if workspace_deps:
            errors.append(
                f"pure root '{crate}' has workspace dependencies: {', '.join(sorted(set(workspace_deps)))}"
            )
    return errors


def check_forbidden_dependencies():
    errors = []
    for name, path, banned in load_forbidden_libs():
        if not banned:
            continue
        r = run(
            [
                "cargo",
                "tree",
                "--edges",
                "normal,no-proc-macro",
                "--manifest-path",
                str(path / "Cargo.toml"),
                "--prefix",
                "none",
            ]
        )
        if r.returncode:
            errors.append(f"{name}: cannot resolve {path}/Cargo.toml — architecture.toml's path is stale")
            continue
        deps = {line.split()[0] for line in r.stdout.splitlines()[1:] if line.strip()}
        violating = sorted(deps & banned)
        if violating:
            errors.append(f"{name} -> {', '.join(violating)}")
    return errors


def dir_layer_of(manifest_path):
    for layer in DIR_LAYERS:
        if f"/{layer}/" in manifest_path:
            return layer
    return None


def check_layering():
    pkgs, edges, seen = {}, [], set()
    for top in DIR_LAYERS:
        for manifest in sorted((ROOT / top).glob("**/Cargo.toml")):
            if "target" in manifest.parts:
                continue
            r = run(["cargo", "metadata", "--no-deps", "--format-version", "1", "--manifest-path", str(manifest)])
            if r.returncode:
                return [f"cannot read {manifest}"]
            for p in json.loads(r.stdout)["packages"]:
                if p["id"] in seen:
                    continue
                seen.add(p["id"])
                layer = dir_layer_of(p["manifest_path"])
                if layer is None:
                    continue
                pkgs[p["name"]] = layer
                edges += [
                    (p["name"], d["name"])
                    for d in p["dependencies"]
                    if d.get("path") and d.get("kind") is None
                ]
    return [
        f"{a}({pkgs[a]}) -> {b}({pkgs[b]})"
        for a, b in edges
        if a in pkgs and b in pkgs and DIR_LAYERS[pkgs[b]] > DIR_LAYERS[pkgs[a]]
    ]


def check_no_concrete_impl():
    errors = []
    for crate, banned in NO_CONCRETE_IMPL.items():
        r = run(["cargo", "tree", "-p", crate, "--edges", "normal", "--prefix", "none"])
        if r.returncode:
            errors.append(f"cannot read the dependency tree of {crate}: {r.stderr.strip()}")
            continue
        deps = {line.split()[0].lower() for line in r.stdout.splitlines()[1:] if line.strip()}
        violating = sorted(deps & banned)
        if violating:
            errors.append(f"{crate} pulls in a concrete implementation: {', '.join(violating)}")
    return errors


def check_no_binaries():
    r = run(["cargo", "metadata", "--no-deps", "--format-version", "1"])
    if r.returncode:
        return [f"cannot read workspace metadata: {r.stderr.strip()}"]
    metadata = json.loads(r.stdout)
    errors = []
    for p in metadata["packages"]:
        if dir_layer_of(p["manifest_path"]) != "crates":
            continue
        for target in p.get("targets", []):
            if "bin" in target.get("kind", []):
                errors.append(f"{p['name']} has a binary target")
                break
    return errors


def check_facade_only_reexports():
    content = FACADE.read_text()
    pattern = re.compile(r"^\s*pub\s+(fn|struct|enum|trait|const|type)\s", re.MULTILINE)
    matches = sorted(set(pattern.findall(content)))
    if matches:
        return [f"facade crate defines new public items: {', '.join(matches)}"]
    return []


def check_build_gmr():
    r = run(["cargo", "build", "-p", "gmr", "--no-default-features"])
    if r.returncode:
        return [f"cargo build -p gmr --no-default-features failed:\n{r.stderr.strip()}"]
    return []


def check_comments_clean():
    files = run(["git", "ls-files", "*.rs"]).stdout.split()
    errors = []
    for f in files:
        if f in EXEMPT_FILES or not any(f.startswith(zone + "/") for zone in CLEAN_ZONES):
            continue
        for n, line in enumerate((ROOT / f).read_text().splitlines(), 1):
            stripped = line.lstrip()
            if stripped.startswith("//"):
                errors.append(f"{f}:{n}  {stripped[:60]}")
    if errors:
        return ["say it with an anchor, not a comment:"] + errors
    return []


def latest_version_tag():
    r = run(["git", "tag", "--list", "v*", "--sort=-v:refname"])
    tags = [t for t in r.stdout.splitlines() if t.strip()]
    return tags[0] if tags else None


def check_version_bump():
    """Patch is CI's alone: .github/workflows/release.yml bumps it on every
    push to main and tags the result, so no PR should ever touch it. A PR
    may only move Cargo.toml's version by claiming a new major or minor
    line by hand — that is a stability promise a commit-message parser
    cannot make on its own, and this project stopped asking one to (a
    squash-merged PR collapsed its `!`-marked commits into one non-`!`
    title, and the parser that used to sit here read that flattened title
    and got the bump size wrong without either Cargo.toml or gate.sh
    noticing).

    So the only two shapes a PR may leave Cargo.toml in are: unchanged, or
    (major, minor) moved strictly past the latest tag with patch reset to 0.
    """
    with open(ROOT / "Cargo.toml", "rb") as f:
        current = tomllib.load(f)["workspace"]["package"]["version"]

    tag = latest_version_tag()
    if tag is None:
        return []
    tag_version = tag.lstrip("v")
    if current == tag_version:
        return []

    cur_major, cur_minor, cur_patch = (int(x) for x in current.split("."))
    tag_major, tag_minor, _ = (int(x) for x in tag_version.split("."))
    if (cur_major, cur_minor) <= (tag_major, tag_minor):
        return [
            f"Cargo.toml version is {current}, latest tag is {tag} — patch is bumped "
            "by CI on merge, not by a PR; a PR may only move major.minor forward"
        ]
    if cur_patch != 0:
        return [
            f"Cargo.toml version {current} opens a new major.minor line past {tag}, "
            "but its patch digit is nonzero — patch belongs to CI, start the line at .0"
        ]
    return []



CONTRACT_CRATES = {
    "gmr_core": "crates/gmr-core",
    "gmr_budget": "crates/gmr-budget",
    "gmr_content": "crates/gmr-content",
}


def block_from(source, open_at):
    depth = 0
    for i in range(open_at, len(source)):
        if source[i] == "{":
            depth += 1
        elif source[i] == "}":
            depth -= 1
            if depth == 0:
                return source[open_at : i + 1]
    return None


def with_attributes(source, start):
    lines = source[:start].splitlines()
    kept = []
    while lines and lines[-1].lstrip().startswith("#["):
        kept.insert(0, lines.pop())
    return kept


def declaration_of(source, name, beside=""):
    m = re.search(rf"^pub (?:struct|enum) {name}\b", source, re.M)
    if m is not None:
        brace = source.index("{", m.start())
        body = block_from(source, brace)
        if body is None:
            return None
        head = source[m.start() : brace]
        return with_attributes(source, m.start()) + (head + body).splitlines()

    m = re.search(rf"^\s*(?:admitted|minted) {name},(.+)$", source, re.M)
    if m is None:
        return None
    validator = m.group(1).strip().rsplit("::", 1)[-1]
    lines = [m.group(0).strip()]
    where = source if f"fn {validator}" in source else beside
    v = re.search(rf"^(?:pub )?fn {re.escape(validator)}\b", where, re.M)
    if v is not None:
        brace = where.index("{", v.start())
        body = block_from(where, brace)
        if body is not None:
            lines += (where[v.start() : brace] + body).splitlines()
    return lines


def contract_types(module):
    """The contract is whatever `contract.rs` re-exports -- read, never restated.

    A second list of these names in this file would be exactly the drift the
    trait roster check exists to stop, one directory over.
    """
    out, unresolved = [], []
    for path, braced, bare in re.findall(
        r"pub use ([\w:]+)::(?:\{([^}]*)\}|(\w+));", module
    ):
        wanted = [n.strip() for n in (braced or bare).split(",") if n.strip()]
        if path.startswith("crate::"):
            files = [ROOT / "crates/gmr-runtime/src" / f"{path.split('::')[1]}.rs"]
        elif path in CONTRACT_CRATES:
            files = sorted((ROOT / CONTRACT_CRATES[path] / "src").rglob("*.rs"))
        else:
            unresolved += [f"{n} (via `{path}`)" for n in wanted]
            continue
        beside = "\n".join(f.read_text() for f in files if f.exists())
        for name in wanted:
            hit = next(
                (
                    (f, declaration_of(f.read_text(), name, beside))
                    for f in files
                    if f.exists() and declaration_of(f.read_text(), name, beside)
                ),
                None,
            )
            if hit is None:
                unresolved.append(f"{name} (looked under `{path}`)")
            else:
                out.append((name, hit[1]))
    return out, unresolved


def contract_shape():
    module = CONTRACT_MODULE.read_text()
    found, unresolved = contract_types(module)
    parts = [f"{name}\n" + "\n".join(l.rstrip() for l in decl) for name, decl in found]
    digest = hashlib.sha256("\n".join(parts).encode()).hexdigest()
    return f"sha256:{digest}", unresolved


def recorded(source):
    def const(name):
        m = re.search(rf'pub const {name}: &str = "([^"]*)"', source)
        return m.group(1) if m else None

    return const("CONTRACT"), const("SHAPE")


def check_contract_shape_is_earned():
    """A contract type may not change shape without the contract saying so.

    D-6 drew the line between what callers are promised (`Instructions` in;
    `Warrant`, `Grounding`, `Verifiability`, `Ref`, `Version` out) and what is
    ours to move (`Footing`, the `Kind` keys, the health aggregates). The line
    is only worth drawing if crossing it is noticed, and adding a field is the
    quiet way to cross it: nothing fails to compile, every test still passes,
    and a consumer that matched exhaustively on last week's shape is broken by
    a diff that never mentions the contract.

    So `contract::SHAPE` is an earned hash, in the sense CLAUDE.md rule 5 uses
    for probe versions -- a hash over every input that can change what callers
    see, not over bytes that happen to sit nearby. It is recomputed here from
    the declarations themselves, so it cannot drift from them: change a field,
    a variant, or a serde tag, and the recorded digest stops matching until
    somebody writes the new one down.

    Recording the digest is not on its own the promise; moving `CONTRACT` is.
    So the second half compares both against the latest tag: a shape that moved
    since the last release without the version moving with it is the case this
    check exists for. It is skipped when there is no tag, and when the module
    did not yet exist at that tag.
    """
    if not CONTRACT_MODULE.exists():
        return [f"the contract module is gone ({CONTRACT_MODULE.relative_to(ROOT)})"]

    source = CONTRACT_MODULE.read_text()
    version, shape = recorded(source)
    if version is None or shape is None:
        return [
            "the contract module must declare both `CONTRACT` and `SHAPE` as "
            "`pub const .. : &str` -- they are what a caller compares against"
        ]

    computed, unresolved = contract_shape()
    errors = [
        f"the contract re-exports `{m}` and no declaration for it was found -- it "
        "moved or was renamed, and the shape stopped being computed over it"
        for m in unresolved
    ]
    if errors:
        return errors
    if computed != shape:
        return [
            f"the contract's shape is {computed} and `SHAPE` still records {shape} "
            "-- a contract type gained, lost, or renamed something. If callers must "
            "know, move `CONTRACT` past "
            f"`{version}` and record the new digest; if they need not, record it alone"
        ]

    tag = latest_version_tag()
    if tag is None:
        return []
    r = run(["git", "show", f"{tag}:crates/gmr-runtime/src/contract.rs"])
    if r.returncode:
        return []
    was_version, was_shape = recorded(r.stdout)
    if was_shape is None or was_shape == shape:
        return []
    if was_version == version:
        return [
            f"the contract's shape moved since {tag} ({was_shape} -> {shape}) and "
            f"`CONTRACT` is still `{version}` -- callers pin that string to know "
            "what they may match on, so a shape that moves under it is a break "
            "they were told did not happen"
        ]
    return []


TYPED_SURFACES = [
    ROOT / "dist" / "npm" / "index.d.ts",
    ROOT / "dist" / "npm" / "index.js",
    ROOT / "console" / "python" / "gmr.pyi",
]


def check_typed_surface_names_the_contract():
    """The published TypeScript says which contract it describes, and means it.

    The addon hands JSON across, so nothing in Rust declares the shapes a
    TypeScript caller matches on -- `index.d.ts` does, by hand, because a
    second declaration of every contract type in Rust would be the drift path
    the binding exists to avoid. That leaves one thing to check mechanically:
    that the file names the version whose shapes it is describing.

    `check_contract_shape_is_earned` already refuses a shape that moves while
    `CONTRACT` stands still. Together the two are the whole guard: a contract
    type cannot change without `CONTRACT` moving, and `CONTRACT` cannot move
    without this file being edited to say so -- at which moment whoever edits
    it is looking at the declarations that have to move with it.
    """
    if not CONTRACT_MODULE.exists():
        return []
    version, _ = recorded(CONTRACT_MODULE.read_text())
    if version is None:
        return []
    out = []
    for path in TYPED_SURFACES:
        where = path.relative_to(ROOT)
        if not path.exists():
            out.append(f"{where} is gone -- it is the published surface's only type declaration")
            continue
        named = set(re.findall(r'"(gmr\.contract\.v\d+)"', path.read_text()))
        if named != {version}:
            out.append(
                f"{where} names {sorted(named) or 'no contract'} and the runtime is "
                f"`{version}` -- a caller pins that string to know which shapes they "
                "may match on"
            )
    return out


PYTHON_DOOR = ROOT / "console" / "python" / "src" / "lib.rs"
PYTHON_STUB = ROOT / "console" / "python" / "gmr.pyi"


def check_python_stub_spells_the_door():
    """The stub's callable surface is the door's, name for name and arg for arg.

    gmr.pyi is a second hand-written declaration of the python door, and the
    contract check above only reads the version string out of it -- which is
    how the stub once said `from_` while the compiled method said `from`, a
    keyword call per the stub raising TypeError with every test passing,
    because the suite only ever called positionally. Signatures drift exactly
    like comments do; this makes the drift fail the build instead.
    """
    if not PYTHON_DOOR.exists() or not PYTHON_STUB.exists():
        return [
            "the python door or its stub is gone -- gmr.pyi is the callable "
            "surface's only declaration"
        ]

    def spelled(block, drop):
        out = {}
        for attrs, name, params in re.findall(
            r"((?:#\[pyo3[^\]]*\]\s*)*)(?:pub )?fn (\w+)\s*\(([^)]*)\)", block, re.S
        ):
            renamed = re.search(r'name\s*=\s*"(\w+)"', attrs)
            out[renamed.group(1) if renamed else name] = [
                a for a in re.findall(r"(\w+)\s*:", params) if a not in drop
            ]
        return out

    def declared(block, drop):
        return {
            name: [
                a for a in re.findall(r"(\w+)\s*:", params) if a not in drop
            ]
            for name, params in re.findall(r"def (\w+)\(([^)]*)\)", block)
        }

    door = PYTHON_DOOR.read_text()
    stub = PYTHON_STUB.read_text()
    methods_src = door.split("#[pymethods]", 1)[1].split("#[pymodule]", 1)[0]
    door_functions = spelled(
        "\n".join(re.findall(r"#\[pyfunction\][^{]*", door)), {"py"}
    )
    door_methods = spelled(methods_src, {"py"})
    stub_class = stub.split("class Gmr", 1)
    stub_functions = declared(stub_class[0], {"self"})
    stub_methods = declared(stub_class[1] if len(stub_class) > 1 else "", {"self"})

    errors = []
    for what, ours, theirs in (
        ("module function", door_functions, stub_functions),
        ("method", door_methods, stub_methods),
    ):
        for name in sorted(set(ours) | set(theirs)):
            if name not in theirs:
                errors.append(
                    f"the door serves {what} `{name}` and gmr.pyi does not declare it"
                )
            elif name not in ours:
                errors.append(
                    f"gmr.pyi declares {what} `{name}` and the door does not serve it"
                )
            elif ours[name] != theirs[name]:
                errors.append(
                    f"{what} `{name}` takes ({', '.join(ours[name])}) at the door and "
                    f"({', '.join(theirs[name])}) in gmr.pyi -- a keyword caller "
                    "trusting the stub is refused at runtime"
                )
    return errors



TRAIT_ROSTERS = {"gmr-store": "crates/gmr-store", "gmr-content": "crates/gmr-content"}


def check_trait_roster():
    """Every public trait a rostered crate defines is named in its CLAUDE.md bullet.

    A roster in prose is a drift path. This repository carried a seven-name
    list in CLAUDE.md and the same seven in memories/layers.md while
    gmr-store held eight, and nothing noticed until somebody read all three
    -- so the boundary went on being decided from a list that was wrong.
    The names stay in CLAUDE.md because that is where the boundary is
    decided; this is what stops them going stale there.

    Testkit traits are excluded: they are doubles for tests, not contracts
    a store implements, and CLAUDE.md does not speak for them.
    """
    errors = []
    claude = (ROOT / "CLAUDE.md").read_text()
    for crate, path in TRAIT_ROSTERS.items():
        bullet = next(
            (l for l in claude.splitlines() if l.startswith(f"- **`{crate}`**")), None
        )
        if bullet is None:
            errors.append(f"CLAUDE.md has no crate-boundary bullet for {crate}")
            continue
        named = set(re.findall(r"`(\w+)`", bullet))
        for f in sorted((ROOT / path / "src").rglob("*.rs")):
            if f.name == "testkit.rs":
                continue
            for t in re.findall(r"^pub trait (\w+)", f.read_text(), re.M):
                if t not in named:
                    errors.append(
                        f"{crate} defines `{t}` ({f.relative_to(ROOT)}) and CLAUDE.md's "
                        f"boundary bullet does not name it — the boundary is decided from "
                        f"that list, so a trait missing from it is a boundary nobody drew"
                    )
    return errors


def check_acceptance_intact():
    """The portal's sentinel exists, says how many steps ran, and CI greps that number.

    This exists because the file was once truncated mid-heredoc by an editing
    pass. `sh` treats an unterminated `<<'EOF'` as delimited by end-of-file, so
    the script still parsed, still exited 0, and tested almost nothing for two
    days. `sh -n` does not catch it; the sentinel does.

    The portal is now small and the promises live in Python, where a truncated
    module raises instead of passing quietly. So this guards the half that can
    still fail that way, and `check_sentinels_still_aimed` guards the way the
    other half can go hollow.
    """
    errors = []
    lines = ACCEPTANCE.read_text().splitlines()

    tail = next((line for line in reversed(lines) if line.strip()), "")
    if not re.match(r'^echo "ACCEPTANCE COMPLETE steps=\$steps"$', tail.strip()):
        errors.append(
            f"acceptance.sh must end with the sentinel echo, found: {tail.strip()[:60]!r}"
        )

    body = "\n".join(lines)
    if not re.search(r"^step\(\) \{ steps=\$\(\(steps \+ 1\)\)", body, re.MULTILINE):
        errors.append(
            "acceptance.sh's step() must increment $steps — the sentinel prints that "
            "counter, and a step() that does not touch it makes the number a constant "
            "and every check below it decorative"
        )

    if "tools/acceptance.py" not in body:
        errors.append(
            "acceptance.sh no longer hands the shipped binary to tools/acceptance.py, "
            "so the packaging steps are all that runs and no promise is checked at all"
        )

    declared = len(re.findall(r"^step ", body, re.MULTILINE))
    workflow = WORKFLOW.read_text()
    expected = re.search(r"ACCEPTANCE COMPLETE steps=(\d+)", workflow)
    if not expected:
        errors.append(
            "the acceptance workflow does not grep for the sentinel, so a script "
            "that silently stops early still goes green"
        )
    elif int(expected.group(1)) != declared:
        errors.append(
            f"acceptance.sh runs {declared} steps but the workflow greps for "
            f"steps={expected.group(1)} — the two must not drift"
        )
    return errors


def check_every_transport_says_what_it_observes():
    """A transport that can say what it emits has to say it, or nothing checks.

    `Derivation.observes` is what lets `Runtime::open` refuse an anchor whose
    rules read a field the probe never reports -- an anchor that observes
    forever, never transitions, and reads as supervised the whole time.

    Three transports **know**: their whole reading comes back through
    `select::pick`, which puts it under one key. Two **relay**: they run somebody
    else's program but carry a declaration written by whoever installed it --
    shell's is in the artifact manifest and inside the version it is addressed
    by, the in-process one's is handed over with the closure. One **cannot**:
    `script` runs an interpreter over a path with nothing describing it.

    Only that last one may write `Observes::Unknown` in its own `resolve`.
    Anywhere else it is a check silently switched off for every anchor behind
    that transport -- which is what shell did while the description sat in a
    recipe file the transport never saw.

    The rule reads the body of `Transport::resolve` rather than the whole file,
    because every one of these has fixtures that construct an `Unknown` and a
    fixture is not an answer. It matches the full signature and not `fn resolve`,
    because `sql.rs` resolves a connection url under that name first and would
    otherwise be checked against the wrong function. And it is a roster on purpose, unlike the trait rosters two
    checks up: nothing in the source distinguishes "cannot say" from "did not
    bother", so the distinction is recorded by a person and compared by a
    machine.
    """
    families = ROOT / "batteries" / "transport" / "src"
    speaks = {"http.rs", "file.rs", "sql.rs", "shell/mod.rs", "inproc.rs"}
    cannot = {"script.rs"}
    errors = []
    for name in sorted(speaks | cannot):
        f = families / name
        if not f.exists():
            errors.append(f"the transport roster names {name}, which is not there")
            continue
        source = f.read_text()
        m = re.search(r"fn resolve\(&self, name: &ProbeName\)", source)
        if m is None:
            errors.append(f"{name} is rostered as a transport and declares no `resolve`")
            continue
        brace = source.index("{", m.start())
        body = block_from(source, brace) or ""
        if "observes:" not in body:
            errors.append(
                f"{name}'s `resolve` names no `observes`, so every anchor behind it opens "
                "without the check that its rules read something the probe reports"
            )
        elif name in speaks and "Observes::Unknown" in body:
            errors.append(
                f"{name}'s `resolve` answers `Observes::Unknown`, and it is rostered as a "
                "transport that either knows or is handed the answer. Saying it does not "
                "know turns the open-time check off for everything behind it"
            )
    return errors


def check_criteria_inside_the_closure():
    """Anything that decides an extractor's answer is hashed with the extractor.

    Rule 5 says a plugin version is an earned hash over everything that can
    change its output. `identity` decides whether a candidate is eligible at
    all, so it is such a thing, and it lives in the per-extractor sources that
    build.rs feeds into the closure.

    Moving it next to `at`/`facts` in lib.rs would read like tidying -- they are
    all lists of item names -- and would quietly take a criterion out of the
    hash. Nothing would then be reported as a swapped instrument, and readings
    taken under different matching criteria would be compared as though they
    were comparable. `at` and `facts` are outside the closure correctly: they
    say what the probe reports, and the CLI uses them to route. `identity` says
    what the probe decides on. The two look alike and belong on opposite sides.
    """
    extractors = ROOT / "packs" / "coding" / "extract" / "src"
    build = (extractors.parent / "build.rs").read_text()
    closure = re.findall(r'^\s*\(\s*"(\w+)",', build, re.MULTILINE)
    errors = []
    for name in closure:
        f = extractors / f"{name}.rs"
        if not f.exists():
            errors.append(f"build.rs hashes `{name}.rs`, which is not there")
        elif "identity:" not in f.read_text():
            errors.append(
                f"{name}.rs declares no `identity`, so either it lost a criterion or "
                "the criterion moved somewhere build.rs does not hash"
            )
    outside = extractors / "lib.rs"
    if "identity" in outside.read_text():
        errors.append(
            "lib.rs mentions `identity`, and build.rs does not hash lib.rs. A criterion "
            "outside the closure never moves the earned hash, so nothing is ever reported "
            "as a swapped instrument and old readings are compared under new criteria"
        )
    return errors


def check_sentinels_still_aimed():
    """Every mutation still points at code that exists, and at a promise that exists.

    The mutation sentinels are what keep the acceptance assertions from going
    hollow, so they have their own way of dying: the code moves, a mutation's
    anchor stops matching anything, and the sentinel silently stops meaning
    what it says. That failure only surfaces on a full `--mutations` run, which
    rebuilds the binary once per mutation and is far too slow to be the thing
    standing between this drift and `main`.

    So it is checked statically here, on every PR, for nothing.
    """
    sys.path.insert(0, str(ROOT / "tools"))
    try:
        from accept import mutations, spec
    except ImportError as e:
        return [f"the acceptance suite does not import: {e}"]

    promises = {s["id"] for s in spec.SCENARIOS}
    errors = []
    for m in mutations.MUTATIONS:
        for promise in m["breaks"]:
            if promise not in promises:
                errors.append(
                    f"mutation `{m['id']}` says it breaks `{promise}`, and no promise "
                    "by that name exists any more"
                )
        if m.get("skip"):
            continue
        source = ROOT / m["file"]
        if not source.exists():
            errors.append(f"mutation `{m['id']}` aims at {m['file']}, which is gone")
        elif m["find"] not in source.read_text():
            errors.append(
                f"mutation `{m['id']}` no longer matches anything in {m['file']} — "
                "the code moved and this sentinel stopped meaning anything. Re-aim it "
                "at what the code does now rather than deleting it"
            )
    return errors


def check_the_old_world_stays_gone():
    """Nothing tracked lives under, or points into, the retired domains/ tree.

    The split renamed domains/ to console/ and packs/ and swore every tool
    that spelled the old paths moved in the same commit. Two did not:
    bench.sh's insides kept copying the addon to domains/node, and one probe
    script stayed behind at the old address with probes.toml pointing at it
    -- each unnoticed because nothing runs them on every push. Residue of a
    rename is the same class of drift as a stale comment, so it fails the
    build instead of waiting to be read.
    """
    old = "dom" + "ains/"
    files = run(["git", "ls-files"]).stdout.splitlines()
    errors = [
        f"`{f}` is tracked under the retired {old} tree -- it moved to packs/ or console/"
        for f in files
        if f.startswith(old)
    ]
    for f in files:
        if f == "tools/gate.py":
            continue
        if not (
            f.startswith((".anchor/", ".github/", "tools/")) or f.endswith(".sh")
        ):
            continue
        text = (ROOT / f).read_text(errors="replace")
        if old in text:
            errors.append(
                f"`{f}` still spells {old} -- the tree it points into was renamed "
                "to packs/ and console/"
            )
    return errors



CHECKS = [
    ("pure roots: zero workspace dependencies", check_pure_roots),
    ("dependency forbidden zones", check_forbidden_dependencies),
    ("layering: lower must not depend on upper", check_layering),
    ("base layer must not ship a concrete implementation", check_no_concrete_impl),
    ("base layer must not produce any binary", check_no_binaries),
    ("facade: only re-exports", check_facade_only_reexports),
    ("every trait a rostered crate defines is named in CLAUDE.md", check_trait_roster),
    ("the contract's shape is the one its version claims", check_contract_shape_is_earned),
("the published types name the contract they describe", check_typed_surface_names_the_contract),
    ("the python stub spells the door's own callable surface", check_python_stub_spells_the_door),
    ("the retired domains/ tree stays gone", check_the_old_world_stays_gone),
    ("facade builds with no default features", check_build_gmr),
    ("no comments in the clean zones", check_comments_clean),
    ("the acceptance sentinel exists and CI checks its count", check_acceptance_intact),
    ("every mutation sentinel still aims at code and at a promise", check_sentinels_still_aimed),
    ("what decides an extractor's answer is hashed with it", check_criteria_inside_the_closure),
    ("every transport that can say what it observes does", check_every_transport_says_what_it_observes),
    ("Cargo.toml version, if touched, only claims a major.minor line — patch is CI's", check_version_bump),
]


def main():
    failed = False
    for label, check in CHECKS:
        print(f"── {label}", flush=True)
        errors = check()
        if errors:
            failed = True
            print("gate: invariant violated", file=sys.stderr)
            for e in errors:
                print(f"  {e}", file=sys.stderr)
            sys.stderr.flush()
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
