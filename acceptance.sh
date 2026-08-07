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
prefix=$work/prefix
repo=$work/repo

fail() {
    echo
    echo "验收失败：$1"
    [ $# -gt 1 ] && { echo "--- 实际输出 ---"; echo "$2"; }
    exit 1
}

step() { echo; echo "── $1"; }

# ── 发布侧：一个二进制。抽取器链在里面，别的什么都不随包走 ───────────────
step "build the tarball (release side; needs cargo)"
cargo build --quiet --release -p coding-anchor
mkdir -p "$bundle/bin"
cp "$root/target/release/gmr" "$bundle/bin/gmr"

# 这个仓库自己的 test-roster 是自举数据，不是产品。发出去就错了。
[ -e "$bundle/probes" ] && fail "tarball 里出现了 probes/ —— 那是这个仓库的自举数据"

step "install the way dist/install.sh does"
mkdir -p "$prefix/bin"
cp "$bundle/bin/gmr" "$prefix/bin/gmr"

gmr="$prefix/bin/gmr"

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
echo "$out" | grep -q "ast-map" || fail "init 没有报告内置探针" "$out"
echo "$out" | grep -q "\.ts" || fail "init 没有认出 TypeScript" "$out"
[ -f "$repo/.anchor/anchors.toml" ] && fail "init 写了锚声明；判据归 owner，不归工具"
[ -f "$repo/.anchor/.gitignore" ] || fail "init 没有写 .anchor/.gitignore"
[ -d "$repo/.anchor/probes" ] && [ -n "$(ls -A "$repo/.anchor/probes" 2>/dev/null)" ] \
    && fail "init 往仓库里复制了探针；抽取器在二进制里，不该有东西落地"

# 身份必须是挣来的，而且必须是跨机器可比的那一种。
out=$("$gmr" --repo "$repo" probes list --json)
echo "$out" | grep -q '"kind":"builtin"' || fail "内置探针没有被列出来" "$out"
echo "$out" | python3 -c '
import json, sys
probes = json.load(sys.stdin)["probes"]
bad = [p["probe"] for p in probes if len(p.get("version") or "") != 64]
sys.exit("not an earned hash: " + ", ".join(bad) if bad else 0)
' || fail "探针版本不是挣来的" "$out"

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

# ── 用户自己的探针：不在代码里的事实 ─────────────────────────────────────
step "a probe the user wrote themselves"
mkdir -p "$repo/scripts"
cat > "$repo/scripts/deploy.sh" <<'EOF'
#!/bin/sh
printf '{"sha":"a1b2c3d"}\n'
EOF
chmod +x "$repo/scripts/deploy.sh"
cat > "$repo/.anchor/probes.toml" <<'EOF'
[script.deploy-sha]
run = "scripts/deploy.sh"
obs = { schema = "gmr.probe-deploy.v1", at = [], facts = ["sha"] }
EOF
cat > "$repo/.anchor/anchors.toml" <<'EOF'
[[anchor]]
key = "deploy::staging"
probe = "deploy-sha"
position = { env = "staging" }
rules = [
  'not exists(state.sha) => { position: state.position, sha: obs.sha, status: "captured" }',
  'obs.sha != state.sha => { position: state.position, sha: obs.sha, was: state.sha, status: "redeployed" }',
]
EOF
printf -- '---\nanchors:\n  - deploy::staging\n---\n\n# staging 上跑的是哪个 commit\n' \
    > "$repo/memories/deploy.md"
(cd "$repo" && git add -A && git -c user.email=a@b -c user.name=t commit -qm deploy)

out=$("$gmr" --repo "$repo" sync)
echo "$out" | grep -q "1 anchors opened" || fail "脚本探针的锚没开出来" "$out"
"$gmr" --repo "$repo" observe >/dev/null

sed -i.bak 's/a1b2c3d/9f8e7d6/' "$repo/scripts/deploy.sh"
set +e
out=$("$gmr" --repo "$repo" observe); code=$?
set -e
[ "$code" -eq 1 ] || fail "部署换了 commit，退出码应当是 1，得到 $code" "$out"
echo "$out" | grep -q "memories/deploy.md" \
    || fail "锚住了源码里看不见的事实，但笔记没有回到手上" "$out"

# ── 门面：一条坐标进去，一个向量出来 ─────────────────────────────────────
step "the front door: anchor / status / check / accept"
cat > "$repo/src/session.ts" <<'EOF'
export function rotate(id: string): string { return id; }
EOF
(cd "$repo" && git add -A && git -c user.email=a@b -c user.name=t commit -qm session)

key='src/session.ts#rotate'
out=$("$gmr" --repo "$repo" anchor "$key" -m '轮换必须在写库之前完成')
echo "$out" | grep -q 'missing · sig · logic · file · line' \
    || fail "anchor 没有从坐标推出 contract 的五个轴" "$out"
echo "$out" | grep -q 'memories/session-rotate.md' || fail "anchor 没有写出笔记" "$out"

out=$("$gmr" --repo "$repo" status "$key")
echo "$out" | grep -q 'contract' || fail "status 没有认出 shape" "$out"
echo "$out" | grep -q 'memories/session-rotate.md' || fail "status 没有列出记忆" "$out"

set +e
out=$("$gmr" --repo "$repo" check "$key"); code=$?
set -e
[ "$code" -eq 0 ] || fail "世界没动时 check 应当是 0，得到 $code" "$out"

# 只移动行号。没人订阅 line，所以记忆不该回到手上，退出码不该是 1。
cat > "$repo/src/session.ts" <<'EOF'
// a
// b
export function rotate(id: string): string { return id; }
EOF
set +e
out=$("$gmr" --repo "$repo" check "$key"); code=$?
set -e
[ "$code" -eq 0 ] || fail "只有没被订阅的轴动了，check 不该报警，得到 $code" "$out"
echo "$out" | grep -q 'memories/session-rotate.md' \
    && fail "没被订阅的轴不该把记忆交出来" "$out"

# 签名、实现、行号一起动。三个都要落地 —— 有序规则表只会报第一个，把另外两个吞掉。
cat > "$repo/src/session.ts" <<'EOF'
// a
// b
// c
export function rotate(id: string, now: number): string {
  const next = id + now;
  return next;
}
EOF
set +e
out=$("$gmr" --repo "$repo" check "$key"); code=$?
set -e
[ "$code" -eq 1 ] || fail "订阅的轴动了，check 应当是 1，得到 $code" "$out"
echo "$out" | grep -q 'memories/session-rotate.md' || fail "check 没有把记忆交回来" "$out"

out=$("$gmr" --repo "$repo" status "$key")
echo "$out" | grep -qE 'sig 1' || fail "签名变了但 sig 位没置起来" "$out"
echo "$out" | grep -qE 'logic 1' || fail "实现变了但 logic 位没置起来 —— 这正是旧表格会吞掉的那个" "$out"
echo "$out" | grep -qE 'line 1' || fail "行号变了但 line 位没置起来" "$out"

out=$("$gmr" --repo "$repo" accept "$key" --why '多传一个 now 不影响“轮换先于写库”这条')
echo "$out" | grep -q 'rationale' || fail "accept 没有封存理由" "$out"

out=$("$gmr" --repo "$repo" status "$key")
echo "$out" | grep -q 'sig 0' || fail "accept 之后 sig 位应当归零" "$out"
set +e
out=$("$gmr" --repo "$repo" check "$key"); code=$?
set -e
[ "$code" -eq 0 ] || fail "accept 之后 check 应当是 0，得到 $code" "$out"


echo
echo "验收通过：陌生仓库、无工具链、零下载探针。记忆与事实连上了，"
echo "          源码里的和源码外的都算，且事实动时记忆回到手上。"
