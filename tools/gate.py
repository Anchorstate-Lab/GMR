#!/usr/bin/env python3
"""Topology and discipline invariants gate.sh's cargo steps cannot express.

Invoked by gate.sh after cargo fmt/clippy/test. Exits 0 if every check below
holds, otherwise prints every violation found (not just the first) and exits 1.
"""

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
CLIFF_TOML = ROOT / "cliff.toml"
FACADE = ROOT / "crates" / "gmr" / "src" / "lib.rs"
ACCEPTANCE = ROOT / "acceptance.sh"
WORKFLOW = ROOT / ".github" / "workflows" / "rust.yml"

PURE_ROOTS = ["gmr-core", "gmr-expr"]

NO_CONCRETE_IMPL = {
    "gmr-probe": {"tokio", "reqwest", "hyper"},
    "gmr-content": {"tokio", "reqwest", "hyper"},
    "gmr-store": {"sqlx", "rusqlite", "libsqlite3", "postgres", "tokio-postgres"},
}

DIR_LAYERS = {"crates": 0, "batteries": 1, "domains": 2}

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
    "domains/coding/extract",
    "domains/coding/cli",
]
EXEMPT_FILES = ["domains/coding/cli/src/cli.rs"]


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
    """Cargo.toml's version is a claim about what the commits since the last
    tag warrant. release-plz used to compute that claim and got it wrong —
    0.3.3 -> 0.3.4 shipped eleven breaking commits as a patch bump — because
    its bump decision depended on `cargo package` succeeding, which this
    workspace's self-referential dev-dependencies never let happen. git-cliff
    needs none of that: it reads commit messages, not packaged crates.

    This only checks the claim when someone is actively making it. On an
    ordinary PR, Cargo.toml's version still matches the latest tag and this
    is a no-op — git-cliff only has to be on PATH for the PR that bumps it.

    The major digit is never checked against git-cliff's answer. Crossing a
    major line is a stability promise, not a fact commit messages can settle
    on their own — a breaking commit only ever justifies a minor bump here
    (see cliff.toml's breaking_always_bump_major = false), and moving off
    that line at all is deliberately left to whoever edits Cargo.toml by hand.
    """
    with open(ROOT / "Cargo.toml", "rb") as f:
        current = tomllib.load(f)["workspace"]["package"]["version"]

    tag = latest_version_tag()
    if tag is None:
        return []
    tag_version = tag.lstrip("v")
    if current == tag_version:
        return []
    if current.split(".", 1)[0] != tag_version.split(".", 1)[0]:
        return []

    try:
        r = run(["git-cliff", "--bumped-version", "--unreleased", "-c", str(CLIFF_TOML)])
    except FileNotFoundError:
        return [
            f"Cargo.toml claims {current} (last tag {tag}) but git-cliff is not on PATH "
            "to verify that bump is the right size"
        ]
    if r.returncode:
        return [f"git-cliff could not compute the expected next version: {r.stderr.strip()}"]
    expected = r.stdout.strip().lstrip("v")
    if not expected:
        return ["git-cliff produced no bumped version to compare against"]
    if current != expected:
        return [
            f"Cargo.toml version is {current}, but commits since {tag} call for {expected} "
            "(git-cliff --bumped-version, breaking_always_bump_major=false)"
        ]
    return []


def check_acceptance_intact():
    """The sentinel exists, says how many steps ran, and CI checks that number.

    This exists because the file was once truncated mid-heredoc by an editing
    pass. `sh` treats an unterminated `<<'EOF'` as delimited by end-of-file, so
    the script still parsed, still exited 0, and tested almost nothing for two
    days. `sh -n` does not catch it.

    What catches it is the sentinel, and only the sentinel. An unterminated
    heredoc swallows every line after it -- including the `step` calls that
    increment the counter and the final echo itself -- so the run either prints
    nothing or prints a number lower than the workflow greps for. This function
    does not read the script for balanced `<<'EOF'` / `EOF` pairs any more. That
    count was a heuristic, it could not see a marker inside a heredoc body (this
    script writes shell scripts into heredocs), and it happened to balance only
    because the paragraph above it is a comment the reader stripped. A check
    that passes for the wrong reason is worse than one that is not there.
    """
    errors = []
    lines = ACCEPTANCE.read_text().splitlines()

    tail = next((line for line in reversed(lines) if line.strip()), "")
    m = re.match(r'^echo "ACCEPTANCE COMPLETE steps=\$steps"$', tail.strip())
    if not m:
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


CHECKS = [
    ("pure roots: zero workspace dependencies", check_pure_roots),
    ("dependency forbidden zones", check_forbidden_dependencies),
    ("layering: lower must not depend on upper", check_layering),
    ("base layer must not ship a concrete implementation", check_no_concrete_impl),
    ("base layer must not produce any binary", check_no_binaries),
    ("facade: only re-exports", check_facade_only_reexports),
    ("facade builds with no default features", check_build_gmr),
    ("no comments in the clean zones", check_comments_clean),
    ("the acceptance sentinel exists and CI checks its count", check_acceptance_intact),
    ("Cargo.toml version bump matches what the commits since the last tag call for", check_version_bump),
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
