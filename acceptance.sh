#!/bin/sh
# 验收标准，一句话：
#
#   陌生用户，在与 GMR 无关的非 Rust 仓库里，没有 Rust 工具链，能把一条记忆连
#   到事实上，并在事实变动时把那条记忆重新拿到手上。
#
# 这个脚本从一个打包好的 bundle 出发跑完整条链路。它**故意不放进 gate.sh**：
# gate.sh 只查基底与电池、不碰任何锚，因为红锚是给人看的信号，不是构建失败。
# 这里断言的是一个 fixture 仓库的锚，那是测试数据，断言它才是本分。
set -eu

root=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
bundle=$work/bundle
repo=$work/repo

fail() {
    echo
    echo "验收失败：$1"
    [ $# -gt 1 ] && { echo "--- 实际输出 ---"; echo "$2"; }
    exit 1
}

step() { echo; echo "── $1"; }

# ── 发布侧：构建二进制与探针，打成 bundle ────────────────────────────────
step "build the bundle (release side; needs cargo)"
cargo build --quiet --release -p coding-anchor
"$root/target/release/gmr" --repo "$root" probes build >/dev/null

mkdir -p "$bundle/bin" "$bundle/probes"
cp "$root/target/release/gmr" "$bundle/bin/gmr"
cp -R "$root/.anchor/probes/." "$bundle/probes/"
cp "$root/.anchor/probes.toml" "$bundle/probes/probes.toml"
[ -f "$bundle/probes/recipes.json" ] || fail "bundle 里没有 recipes.json —— 用户机器算不出配方版本"

gmr="$bundle/bin/gmr"

# ── 用户侧：一个普通 TS 仓库。没有 Rust 源码，没有 Cargo.toml ──────────────
step "a stranger's TypeScript repo"
mkdir -p "$repo/src"
cat > "$repo/src/auth.ts" <<'EOF'
export function createSession(userId: string, ttl: number): Session {
  return { userId, ttl };
}
export const verify = (token: string) => token.length > 0;
EOF
(cd "$repo" && git init -q . && git add -A \
    && git -c user.email=a@b -c user.name=t commit -qm init)

[ -f "$repo/Cargo.toml" ] && fail "fixture 仓库不该有 Cargo.toml"

step "init"
out=$("$gmr" --repo "$repo" init)
echo "$out" | grep -q "ast-map" || fail "init 没有装上预装探针" "$out"
echo "$out" | grep -q "\.ts" || fail "init 没有认出 TypeScript" "$out"
[ -f "$repo/.anchor/anchors.toml" ] && fail "init 写了锚声明；判据归 owner，不归工具"
[ -f "$repo/.anchor/.gitignore" ] || fail "init 没有写 .anchor/.gitignore"

# 配方版本必须来自 recipes.json：这个仓库没有探针源码，算不出来。
out=$("$gmr" --repo "$repo" probes list --json)
echo "$out" | grep -q '"pinned":true' || fail "配方版本不是固定的，说明它去哈希源码了" "$out"

# ── 一行 frontmatter。用户不写 anchors.toml，不写转换表 ────────────────────
step "one line of frontmatter"
printf -- '---\nabout: src/auth.ts#createSession\n---\n\n# 会话只在服务边界内创建\n' \
    > "$repo/memories/auth.md"
(cd "$repo" && git add -A && git -c user.email=a@b -c user.name=t commit -qm note)

out=$("$gmr" --repo "$repo" sync)
echo "$out" | grep -q "1 anchors opened" || fail "笔记没有开出锚" "$out"
echo "$out" | grep -q "memories/auth.md" || fail "笔记没有被绑定" "$out"

# 再跑一次不该写任何东西：绑定表只增不改。
out=$("$gmr" --repo "$repo" sync)
echo "$out" | grep -q "memories/auth.md" && fail "sync 不是幂等的，重复追加了绑定" "$out"

step "observe: the world has not moved"
set +e
out=$("$gmr" --repo "$repo" observe); code=$?
set -e
[ "$code" -eq 0 ] || fail "世界没动时退出码应当是 0，得到 $code" "$out"

# ── 事实动了 ──────────────────────────────────────────────────────────────
step "the world moves"
cat > "$repo/src/auth.ts" <<'EOF'
export function createSession(userId: string, ttl: number, scope: string): Session {
  return { userId, ttl, scope };
}
export const verify = (token: string) => token.length > 0;
EOF

set +e
out=$("$gmr" --repo "$repo" observe); code=$?
set -e
[ "$code" -eq 1 ] || fail "世界动了时退出码应当是 1，得到 $code" "$out"
echo "$out" | grep -q "moved" || fail "没有报告 moved" "$out"
# 这一行就是验收标准本身：事实动了，笔记回到手上。
echo "$out" | grep -q "memories/auth.md" \
    || fail "锚动了，但没有交出绑定在它上面的笔记 —— 这是整个产品的价值所在" "$out"

step "pass --json: the shape an agent loop reads"
"$gmr" --repo "$repo" requeue 'src/auth.ts#createSession' >/dev/null
cat > "$repo/src/auth.ts" <<'EOF'
export function createSession(userId: string): Session { return { userId }; }
EOF
set +e
out=$("$gmr" --repo "$repo" pass --json); code=$?
set -e
[ "$code" -eq 1 ] || fail "pass 在有锚移动时退出码应当是 1，得到 $code" "$out"
echo "$out" | grep -q '"memories":\["memories/auth.md"\]' \
    || fail "pass --json 没有把笔记交回来" "$out"

echo
echo "验收通过：陌生仓库、无工具链，记忆与事实连上了，且事实动时记忆回到手上。"
