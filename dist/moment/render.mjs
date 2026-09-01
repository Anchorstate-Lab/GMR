#!/usr/bin/env node
// One report, many moments. Reads `gmr check --json` and `gmr standing --json`
// and renders the markdown every adapter shares — the PR comment, the pre-push
// output, the job summary. Zero dependencies; bodies are fetched through the
// gmr binary itself when --gmr is given, so this file never parses a note.

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

const args = process.argv.slice(2);
const opt = (name, fallback) => {
  const at = args.indexOf(`--${name}`);
  return at >= 0 ? args[at + 1] : fallback;
};

const check = JSON.parse(readFileSync(opt("check", "check.json"), "utf8"));
const standing = JSON.parse(readFileSync(opt("standing", "standing.json"), "utf8"));
const gmr = opt("gmr", null);
const cap = Number(opt("cap", "2000"));

const lines = [];
const say = (s = "") => lines.push(s);

const bodyOf = (anchor) => {
  if (!gmr) return [];
  try {
    const out = JSON.parse(
      execFileSync(gmr, ["read", anchor, "--json"], { encoding: "utf8", maxBuffer: 16e6 }),
    );
    return (out[0]?.memories ?? []).flatMap((m) => {
      const content = m.grounding?.content;
      if (typeof content !== "string") return [];
      const name = m.reference?.external_id ?? "?";
      const text = content.length > cap ? `${content.slice(0, cap)}\n…(truncated)` : content;
      return [{ name, text }];
    });
  } catch {
    return [];
  }
};

const due = check.handed_back ?? [];
const unstood = standing.filter((s) => {
  const broken = ["broken", "vacuous", "unevaluable"].includes(s.depends);
  const ground = (s.on ?? []).some(
    (a) => a.anchored === "on" && a.warrant?.holding?.holding !== "holds",
  );
  const unseen = (s.on ?? []).some((a) => a.evidence?.shown === "unseen");
  return broken || ground || unseen;
});

say("### ⚓ gmr — what this change moves");
say();

if (due.length === 0 && unstood.length === 0) {
  say(
    `**${check.observed} anchors observed · quiet.** Nothing bound to this ` +
      `change is due for re-reading — that silence is the product working, ` +
      `not the product idle.`,
  );
} else {
  if (due.length > 0) {
    say(`**${due.length} anchor(s) moved on a watched axis — the memories below are due.**`);
    say(`A moved memory is not false; it is due: re-read it, then \`gmr accept <key> --why\`.`);
    say();
    for (const h of due) {
      say(`#### \`${h.anchor}\`  —  ${h.status}`);
      if (h.diagnosis) say(`> ${h.diagnosis}`);
      for (const { name, text } of bodyOf(h.anchor)) {
        say(`<details><summary><code>${name}</code></summary>`);
        say();
        say("```markdown");
        say(text);
        say("```");
        say("</details>");
      }
      if (!gmr) for (const m of h.memories ?? []) say(`- ${m}`);
      say();
    }
  }
  if (unstood.length > 0) {
    say(`**${unstood.length} recorded conclusion(s) no longer stand.**`);
    for (const s of unstood) {
      const id = s.claim?.said ?? s.claim?.external_id ?? JSON.stringify(s.claim);
      say(`- \`${id}\` — depends: ${s.depends}`);
    }
    say();
  }
}

const warn = (label, items, shape) => {
  if (!items?.length) return;
  say(`**${label}** (${items.length})`);
  for (const it of items) say(`- ${shape(it)}`);
  say();
};
say();
warn("criteria drifted — declaration and live criteria disagree", check.criteria_drifted, (d) => `\`${d.anchor}\`: ${d.facets}`);
warn("criteria unreadable", check.criteria_unreadable, (d) => `\`${d.anchor}\`: ${d.reason}`);
warn("supervised by no note", check.criteria_undeclared, (k) => `\`${k}\``);
warn("instrument swapped since baseline", check.instrument_swapped, (d) => `\`${d.anchor}\``);
warn("could not observe", check.unseen, (d) => `\`${d.anchor}\`: ${d.detail}`);
warn("watch invalid", check.watch_invalid, (d) => `${d.note}: ${d.detail}`);

if (check.moved_unwatched > 0) {
  say(
    `_${check.moved_unwatched} anchor(s) moved only on axes no note subscribes to — ` +
      `by design nothing is handed back; \`gmr status\` shows them._`,
  );
}

process.stdout.write(lines.join("\n") + "\n");
process.exit(0);
