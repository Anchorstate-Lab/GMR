#!/bin/sh
# **类型和测试表达不了的恒等式，唯一的住处。** 这个脚本原样就是 CI。
#
#   sh gate.sh
#
# 恒等式 = 一次为真、应当永远为真的关系。它不需要有人接收信号 —— 所以它不该
# 住在散文、注释或记忆里，那三处都要等一个人读到才生效，而恒等式被破坏的那一
# 刻没有任何东西会动。恒等式有三个家，按「谁能表达它」分：
#
#   类型      能让它不可能被违反的，优先（同一个值只存一份，就没有两份可分家）
#   测试      Rust 能判定的，住 `cargo test`，紧挨着它保护的那段代码
#   gate.sh   跨包的依赖事实、文件层面的文本纪律 —— 前两者都够不着的
#
# 这里只有两节：拓扑、纪律。加一条新的恒等式先问前两个家收不收；都不收才进来。
#
# 它查的是**源码树**，**不碰任何锚**。锚报出的状态是要有人来看的信号，不是
# 构建失败 —— 门禁替它判，就等于把语义搬进了工具。判据漂了这类关于**运行时
# 存储**的恒等式归 `gmr check`，不归这里。
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

echo
echo "══ 拓扑"

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

echo
echo "══ 纪律"

# 注释和记忆各存一份必然分家，而记忆有锚盯着、注释没有。清区只增不减：
# 清完一个包就往 CLEAN 加一行，那一行是一次真实清理的凭证。不做 diff 比对 ——
# diff 要一个 base ref，CI 里那是状态；清区名单零状态、单调。
#
# EXEMPT 是 CLAUDE.md 写明的两个例外里的第二个：clap 的 `///` 是 --help 的
# 正文，是给用户看的字符串，只是碰巧用了注释语法。第一个例外 `//!` 说的是
# 「这个文件是什么」，直接在判据里放行。
echo "── 清区里零注释"
python3 - <<'COMMENTS' || exit 1
import pathlib, subprocess, sys

CLEAN = ["crates/gmr-core", "crates/gmr-content", "crates/gmr"]
EXEMPT = ["domains/coding/cli/src/cli.rs"]

files = subprocess.run(["git", "ls-files", "*.rs"],
                       capture_output=True, text=True, check=True).stdout.split()
bad = []
for f in files:
    if f in EXEMPT or not any(f.startswith(d + "/") for d in CLEAN):
        continue
    for n, line in enumerate(pathlib.Path(f).read_text().splitlines(), 1):
        s = line.lstrip()
        if s.startswith("//") and not s.startswith("//!"):
            bad.append(f"{f}:{n}  {s[:60]}")
if bad:
    print("gate: 清区里出现了注释 —— 要说的话锚成记忆",
          *bad, sep="\n  ", file=sys.stderr)
    sys.exit(1)
COMMENTS

echo
echo "gate: 恒等式全绿"
