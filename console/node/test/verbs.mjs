import { createRequire } from "node:module";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import assert from "node:assert/strict";

const require = createRequire(import.meta.url);
const native = require(process.env.GMR_ADDON ?? "../gmr.node");

function aRepository() {
  const root = mkdtempSync(join(tmpdir(), "gmr-node-"));
  execFileSync("git", ["init", "-q"], { cwd: root });
  mkdirSync(join(root, "envs"), { recursive: true });
  mkdirSync(join(root, "memories"), { recursive: true });
  writeFileSync(join(root, "envs", "prod.yaml"), "service:\n  replicas: 9\n");
  writeFileSync(
    join(root, "memories", "replicas.md"),
    "Nine, because eight cannot survive a rolling restart.\n",
  );
  return root;
}

async function opened(root) {
  return native.open({
    root,
    providers: { git: true },
    recipes: {
      file: { replicas: { path: "envs/{env}.yaml", select: "$.service.replicas" } },
    },
  });
}

test("five lines get a sentence's grounding", async () => {
  const root = aRepository();
  const gmr = await opened(root);

  await gmr.open({
    key: "prod-replicas",
    probe: { kind: "file", name: "replicas" },
    initial: { position: { env: "prod" } },
    transitions: [
      { when: "true", to: "{ position: state.position, v: obs.value }" },
    ],
  });

  await gmr.bind("git:memories/replicas.md", ["prod-replicas"], "derived");

  const [standing] = await gmr.ground(["git:memories/replicas.md"], {
    max_staleness_ms: 0,
  });

  assert.equal(standing.claim.provider, "git");
  assert.equal(standing.on.length, 1, "the sentence is about one anchor");
  assert.equal(standing.on[0].anchored, "on");
  assert.equal(standing.on[0].key, "prod-replicas");
  assert.ok(standing.on[0].warrant, "and the anchor says whether it still holds");
  assert.ok(
    standing.on[0].evidence.reading,
    "with an address for what was read, and no value: a caller that wants the fact " +
      "asks the fact layer for it, and one that wants to audit the judgement follows " +
      "the address",
  );
  assert.equal(standing.on[0].evidence.value, undefined);

  const declared = ["current", "unverified", "rewritten", "gone", "no_provider", "unreachable"];
  assert.ok(
    declared.includes(standing.record.grounding),
    "index.d.ts is hand-written, because the alternative is declaring every " +
      "contract type a second time in Rust. So the discriminants it names have " +
      "to be walked by something: " + JSON.stringify(standing.record),
  );
  assert.ok(
    ["holds", "moved", "incomparable", "absent", "never_established", "undated"]
      .includes(standing.on[0].warrant.holding.holding),
    JSON.stringify(standing.on[0].warrant),
  );
  assert.ok(
    ["seen", "blind"].includes(standing.on[0].warrant.knowledge.knowledge),
    JSON.stringify(standing.on[0].warrant),
  );
});

test("what changed since a cursor comes back as edges", async () => {
  const root = aRepository();
  const gmr = await opened(root);
  await gmr.open({
    key: "prod-replicas",
    probe: { kind: "file", name: "replicas" },
    initial: { position: { env: "prod" } },
  });

  const seen = await gmr.since(0);
  assert.ok(Array.isArray(seen.edges));
  assert.ok(seen.cursor >= 0);
});

test("an instruction nobody here understands is refused, not dropped", async () => {
  const root = aRepository();
  const gmr = await opened(root);
  await assert.rejects(
    () => gmr.ground(["git:memories/replicas.md"], { maxStaleness: 60000 }),
    /maxStaleness/,
    "a freshness bound silently dropped is an answer served stale under a bound the " +
      "caller believes they set",
  );
});

test("an address that names no store is refused before anything is asked", async () => {
  const root = aRepository();
  const gmr = await opened(root);
  await assert.rejects(
    () => gmr.ground(["memories/replicas.md"]),
    /^.*refused: .*names nothing/,
    "the kind travels in front of the prose, so a caller matches a token, not a sentence",
  );
});

test("read hands back the envelope, and a carried edge says who asserted it", async () => {
  const root = aRepository();
  const gmr = await opened(root);
  await gmr.open({
    key: "prod-replicas",
    probe: { kind: "file", name: "replicas" },
    initial: { position: { env: "prod" } },
    transitions: [
      { when: "true", to: "{ position: state.position, v: obs.value }" },
    ],
  });
  await gmr.bind("git:memories/replicas.md", ["prod-replicas"], "derived");
  await gmr.link("git:memories/replicas.md", "git:memories/why.md", "rests-on", "adjudicated");

  const view = await gmr.read("prod-replicas");
  assert.equal(view.key, "prod-replicas");
  assert.equal(view.memories.length, 1, "carry is opt-in; only the bound record answers");
  const held = view.memories[0];
  assert.equal(held.grounded, true);
  assert.equal(held.links.length, 1);
  assert.equal(held.links[0].kind, "rests-on");
  assert.equal(held.links[0].source, "adjudicated");
  assert.ok(held.warrant, "a bound record carries how it stands to this anchor");

  const dropped = await gmr.unlink("git:memories/replicas.md", "git:memories/why.md", "rests-on", "adjudicated");
  assert.equal(dropped, 1);
  const after = await gmr.read("prod-replicas");
  assert.equal(after.memories[0].links.length, 0, "a revoked edge stops being served");
});
