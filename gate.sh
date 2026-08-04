#!/bin/sh
# **基底的门禁。** 这个脚本原样就是 CI。
#
#   sh gate.sh
#
# 它只查基底与电池,**不碰任何锚**。锚报出的状态是要有人来看的信号，
# 不是构建失败 —— 门禁替它判，就等于把语义搬进了工具。
set -e
cd "$(dirname "$0")"

echo "── clean"
cargo clean

echo "── fmt"
cargo fmt --all --check

echo "── clippy (-D warnings)"
cargo clippy --workspace --all-targets -- -D warnings

echo "── test"
cargo test --workspace

# 纯度不靠纯脸/脏脸的 trait 分裂,由 crate 边界扛。但依赖清单是一条
# **外部检查**,不是类型约束 —— 不在这里查,就没人会查。
echo "── 纯根：零 workspace 依赖"
for c in gmr-core gmr-expr; do
  if cargo tree -p "$c" --edges normal --prefix none | tail -n +2 | grep -qE '^gmr-'; then
    echo "gate: $c 有 workspace 依赖 —— 它不再是纯根" >&2
    exit 1
  fi
done

echo "── 依赖禁区（清单只有 architecture.toml 一份，不再各存拷贝）"
python3 - <<'FORBIDDEN' || exit 1
import tomllib, subprocess, sys
arch = tomllib.load(open("architecture.toml", "rb"))
bad = []
errs = []
for m in arch["member"]:
    if m.get("kind") != "package":
        continue
    keys = (arch["layer"][m["layer"]].get("forbidden", [])
            + m.get("forbidden", []) + m.get("forbidden_default", []))
    banned = {n for k in keys for n in arch["libs"][k]}
    if not banned:
        continue
    # `-p <name>` only resolves inside the workspace gate.sh happens to be
    # run from; members that live in a *different* workspace (batteries/
    # probes/ is its own) would silently and permanently pass. Go straight
    # to the member's own manifest instead — that resolves regardless of
    # which workspace it belongs to.
    r = subprocess.run(["cargo", "tree", "--edges", "normal,no-proc-macro",
                        "--manifest-path", f"{m['path']}/Cargo.toml", "--prefix", "none"],
                       capture_output=True, text=True)
    if r.returncode:
        errs.append(f"{m['name']}: cannot resolve {m['path']}/Cargo.toml — "
                     "architecture.toml's path is stale")
        continue
    deps = {l.split()[0] for l in r.stdout.splitlines()[1:] if l.strip()}
    bad += [f"{m['name']} -> {d}" for d in sorted(deps & banned)]
if errs:
    print("gate: 禁区检查够不到这些成员", *errs, sep="\n  ", file=sys.stderr)
    sys.exit(1)
if bad:
    print("gate: 依赖禁区被撞了", *bad, sep="\n  ", file=sys.stderr)
    sys.exit(1)
FORBIDDEN

echo "── 分层：底层不许依赖上层"
python3 - <<'LAYERS' || exit 1
import json, pathlib, subprocess, sys
LAYER = {"crates": 0, "batteries": 1, "domains": 2}
pkgs, edges, seen = {}, [], set()
for top in LAYER:
    for man in sorted(pathlib.Path(top).glob("**/Cargo.toml")):
        if "target" in man.parts:
            continue
        r = subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1",
                            "--manifest-path", str(man)], capture_output=True, text=True)
        if r.returncode:
            print(f"gate: 读不动 {man}", file=sys.stderr)
            sys.exit(1)
        for p in json.loads(r.stdout)["packages"]:
            if p["id"] in seen:
                continue
            seen.add(p["id"])
            lay = next((k for k in LAYER if f"/{k}/" in p["manifest_path"]), None)
            if lay is None:
                continue
            pkgs[p["name"]] = lay
            edges += [(p["name"], d["name"]) for d in p["dependencies"]
                      if d.get("path") and d.get("kind") is None]
bad = [f"{a}({pkgs[a]}) -> {b}({pkgs[b]})" for a, b in edges
       if a in pkgs and b in pkgs and LAYER[pkgs[b]] > LAYER[pkgs[a]]]
if bad:
    print("gate: 底层依赖了上层", *bad, sep="\n  ", file=sys.stderr)
    sys.exit(1)
LAYERS

echo "── 基底不 ship 任何具体实现"
if cargo tree -p gmr-probe --edges normal --prefix none | tail -n +2 |
   grep -qiE '^(tokio|reqwest|hyper)'; then
  echo "gate: gmr-probe 拖进了传输的实现 —— 它该只是契约" >&2
  exit 1
fi
if cargo tree -p gmr-store --edges normal --prefix none | tail -n +2 |
   grep -qiE '^(sqlx|rusqlite|libsqlite3|postgres|tokio-postgres)'; then
  echo "gate: gmr-store 默认就把数据库拖进来了" >&2
  exit 1
fi

echo "── 基底不产二进制"
if cargo metadata --no-deps --format-version 1 |
   grep -oE '"name":"gmr[^"]*","[^}]*"kind":\["bin"\]' | grep -q .; then
  echo "gate: 基底里冒出了一个二进制 —— 装配是域的事" >&2
  exit 1
fi

echo "── 门面：只重导出"
if grep -qE '^pub (fn|struct|enum|trait|const|type) ' crates/gmr/src/lib.rs; then
  echo "gate: 门面里出现了定义 —— 它成了第七个包,而且是没人看守的那个" >&2
  exit 1
fi
cargo build -p gmr --no-default-features

# **探针实现自成 workspace，所以 --workspace 扫不到它们。** 一条命令点名
# 整个 batteries/probes/ workspace，新增 member 自动被 --workspace 覆盖，
# 不用再在这里逐个加名字。
echo "── 电池（独立 workspace）：batteries/probes"
cargo fmt --manifest-path batteries/probes/Cargo.toml --all --check
cargo clippy --quiet --manifest-path batteries/probes/Cargo.toml --all-targets -- -D warnings
cargo test --quiet --manifest-path batteries/probes/Cargo.toml

echo
echo "gate: 基底全绿"
