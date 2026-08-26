---
anchors:
  - key: layer::gmr-budget
    probe: ast-map
    params: { root: crates/gmr-budget }
    position: { vis: "pub" }
    shape: roster
  - key: layer::gmr-core
    probe: ast-map
    params: { root: crates/gmr-core }
    position: { vis: "pub" }
    shape: roster
  - key: layer::gmr-expr
    probe: ast-map
    params: { root: crates/gmr-expr }
    position: { vis: "pub" }
    shape: roster
  - key: layer::gmr-probe
    probe: ast-map
    params: { root: crates/gmr-probe }
    position: { vis: "pub" }
    shape: roster
  - key: layer::gmr-content
    probe: ast-map
    params: { root: crates/gmr-content }
    position: { vis: "pub" }
    shape: roster
  - key: layer::gmr-store
    probe: ast-map
    params: { root: crates/gmr-store }
    position: { vis: "pub" }
    shape: roster
  - key: layer::gmr-runtime
    probe: ast-map
    params: { root: crates/gmr-runtime }
    position: { vis: "pub" }
    shape: roster
  - key: layer::gmr
    probe: ast-map
    params: { root: crates/gmr }
    position: { vis: "pub" }
    shape: roster
watch: [grew, shrank, roll]
---

# A layer's public surface widening *is* that layer's responsibility changing

Fine-grained anchors answer "what does this piece of code guard". However many of
them there are, they cannot answer the other question: **what is this layer now.**
One more `pub` in a crate and no single-point anchor moves — and that is precisely
the contract between two layers changing.

These eight anchors watch the public-surface roster of eight crates. Four axes, of
which `watch` subscribes to the first three:

```
grew     the list got longer     does the new thing belong to this layer
shrank   the list got shorter    who still depends on what left
roll     the members changed     some in, some out; ask about both
missing  the coordinate hit nothing    the anchor points at the wrong place, or the crate is gone
```

`grew` / `shrank` measure the **net direction relative to the baseline**, not "what
happened". Add two and delete one and only `grew` lights up — that deletion is
reported by `roll`, and finding out who it was means a human comparing the
`baseline.roll` and `now.roll` lists. The evaluator has no set difference, so that
step can only be done by a person, which is why the roster stores **readable
names** and not hashes: saving space would turn this anchor into a question nobody
can answer.

**This layer looks at nothing about what a member is like.** Change a pub
function's implementation, move it, change its signature — not one of these four
axes moves. Those belong to the fine-grained anchors; the two layers each mind
their own, and deleting either one leaves a gap the other cannot fill.

## When the list changes, ask by that layer's entry test

| layer | may only hold | if anything else shows up, ask |
|---|---|---|
| `gmr-budget` | `Budget` and `Spent` | has anything else arrived — it is named by crates that want the vocabulary and not the contract, and every addition is something they now have to take |
| `gmr-core` | vocabulary · content addresses · Entry · fold | is it starting to know how facts are fetched / how rules are computed / how things are stored |
| `gmr-expr` | pure expression evaluation | is there IO · a clock · a dependency on gmr-core |
| `gmr-probe` | the probe invocation contract | has a concrete transport implementation crept in |
| `gmr-content` | the retrieval and discovery contracts | is it a concrete provider (that belongs to a battery), or has a required method appeared that only some stores can honour — that one belongs in its own trait, not in `ContentProvider` |
| `gmr-store` | storage traits and feature-gated backends | can a store refuse it and still be a complete store — if not, it is a contract and is sliced by **mutability**; if so, it is a capability it declines by not implementing |
| `gmr-runtime` | the one orchestration layer | is it starting to make the domain's judgments for it |
| `gmr` | re-exports only | any definition of its own is out of bounds |

The tests come from CLAUDE.md's crate-boundary section. This does not restate it;
it wires it to an anchor that can speak — a boundary written in a document waits
for someone to remember to go and read it, while a boundary hung on an anchor
hands this table back on the day it is crossed.

That claim was once false. The `gmr-store` row carried the same seven-name trait
roster CLAUDE.md carried, both went stale when an eighth arrived, and the entry
test then asked a question the list could no longer answer — which is how a
boundary gets decided from a roster that is wrong. The rosters are CLAUDE.md's
alone now and `tools/gate.py` compares them against the crates, so what is left
in this column is the part a machine cannot decide: the question to ask.

`gmr-content` was added to both late, and the gap is the argument for this anchor
rather than a footnote to it. That crate held three public items and appeared in
neither CLAUDE.md §5 nor this list. Three commits then took it to nine — `History`
split out of `ContentProvider`, `Claim` / `Record` / `MemorySource` added, an error
code made serialisable — and **not one of them moved a single anchor here**, which
is precisely the sentence at the top of this file describing what these anchors
exist to catch. A layer with no anchor does not report a quiet surface; it reports
nothing, and the two are indistinguishable from the outside.

## Why `params` + `vis` rather than a coordinate

`about: <path>` can only name one file. A whole layer needs the long-hand form:
`params: { root: <crate> }` narrows the probe's field of view to this crate, and
`position: { vis: "pub" }` picks out the public surface. That is exactly the "the
probe needs to look at something other than the repository root" case among the
four escape-hatch reasons; see [[memories-lint]].

The `gmr` layer's roster is almost entirely `import:` entries — its public surface
**is** those `pub use`s. Those entries once had no identity at all (a
`use_declaration` carries no `name` field); the fix was to give them real names,
not to invent a fallback id that could never be stable. See [[ast-signature]].

## When this changes, ask

`missing` added to `watch` → think it through: should a whole crate disappearing be
reported by this anchor, or should the anchor be closed? Not subscribed by default,
because that usually means the anchor itself should be pointed somewhere else.

`grew` showing up on `gmr` → a direct violation of decision 12; that layer only
re-exports.

Some layer's roster changing **drastically** → first ask whether a package was
split or renamed. That case calls for opening a new generation of the anchor and
`supersedes`-ing the old one (see [[anchor-Superseded]]), not accepting the
difference as drift.

Writing the criteria as `watch` rather than into this prose is deliberate: prose
that is wrong is never noticed, while a misspelled axis name in `watch` — `sync`,
say — is an error on the spot. See [[shapes-Dim]].
