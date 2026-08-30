import { createRequire } from "node:module";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const require = createRequire(import.meta.url);
const native = require(process.env.GMR_ADDON ?? "../gmr.node");

const ANCHORS = Number(process.env.GMR_BENCH_ANCHORS ?? 1);
const WARM = Number(process.env.GMR_BENCH_WARM ?? 300);
const LOOKS = Number(process.env.GMR_BENCH_LOOKS ?? 100);

function repository() {
  const root = mkdtempSync(join(tmpdir(), "gmr-bench-"));
  execFileSync("git", ["init", "-q"], { cwd: root });
  mkdirSync(join(root, "envs"), { recursive: true });
  mkdirSync(join(root, "memories"), { recursive: true });
  writeFileSync(join(root, "envs", "prod.yaml"), "service:\n  replicas: 9\n");
  writeFileSync(join(root, "memories", "replicas.md"), "Nine, and here is why.\n");
  return root;
}

function spread(taken) {
  const sorted = [...taken].sort((a, b) => a - b);
  const at = (q) => sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))];
  return { p50: at(0.5), p95: at(0.95), max: sorted[sorted.length - 1] };
}

async function timed(times, work) {
  await work();
  const taken = [];
  for (let i = 0; i < times; i += 1) {
    const at = process.hrtime.bigint();
    await work();
    taken.push(Number(process.hrtime.bigint() - at) / 1e6);
  }
  return spread(taken);
}

function say(what, { p50, p95, max }) {
  const ms = (n) => n.toFixed(3).padStart(8);
  console.log(`${what.padEnd(46)} p50 ${ms(p50)} ms   p95 ${ms(p95)} ms   max ${ms(max)} ms`);
}

const root = repository();
const gmr = await native.open({
  root,
  providers: { git: true },
  recipes: { file: { replicas: { path: "envs/{env}.yaml", select: "$.service.replicas" } } },
});

const keys = [];
for (let i = 0; i < ANCHORS; i += 1) {
  const key = `replicas-${i}`;
  await gmr.open({
    key,
    probe: { kind: "file", name: "replicas" },
    initial: { position: { env: "prod" } },
    transitions: [{ when: "true", to: "{ position: state.position, v: obs.value }" }],
  });
  keys.push(key);
}
const address = "git:memories/replicas.md";
await gmr.bind(address, keys, "derived");

const seen = await gmr.sample(keys[0], { max_staleness_ms: 0 });
const said = "said:turn-1";
await gmr.bind(said, keys, "self_attested", {
  saw: seen.fact_address,
  asserts: { answer: "three replicas" },
  depends: "all(anchors, exists(state.v))",
});

const blind = await native.open({ root, providers: {} });

console.log(`gmr node addon — ${ANCHORS} anchor(s) on one sentence, one sqlite store\n`);
say("ground, no store wired (journal + fold only)", await timed(WARM, () => blind.ground([address])));
say("ground, served from the record", await timed(WARM, () => gmr.ground([address])));
say(
  "ground, forced to look again",
  await timed(LOOKS, () => gmr.ground([address], { max_staleness_ms: 0 })),
);
say(
  "ground a said: claim (nothing to fetch)",
  await timed(WARM, () => gmr.ground([said])),
);
say(
  "ground a said: claim, invariant + shown",
  await timed(WARM, () => gmr.ground([said], { max_staleness_ms: 0 })),
);
say(
  "ground, following links 3 hops",
  await timed(WARM, () => gmr.ground([address], { reach: 3 })),
);
say("sample one anchor, forced to look", await timed(LOOKS, () => gmr.sample(keys[0], { max_staleness_ms: 0 })));
say("since(0), every anchor's record fetched", await timed(WARM, () => gmr.since(0)));
say("since(0, status), no record fetched", await timed(WARM, () => gmr.since(0, "any")));
