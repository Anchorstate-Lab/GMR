---
about:
  - domains/coding/cli/src/verbs/anchor.rs#fetched
  - domains/coding/cli/src/verbs/anchor.rs#derive_name
  - domains/coding/cli/src/verbs/anchor.rs#slug
  - domains/coding/cli/src/verbs/anchor.rs#fetch_declared
  - domains/coding/cli/src/probes.rs#HttpDecl
  - domains/coding/cli/src/probes.rs#Declared
  - domains/coding/cli/src/probes.rs#declare_http
watch: [sig, logic]
---

# A URL is not a coordinate; it is something a coordinate gets generated from

D-9. `gmr anchor 'https://crates.io/api/v1/crates/serde#$.crate.max_stable_version'`
works in one command, with no `probes.toml` to edit first and no `probes build`.
What it writes is two declarations a person can read:

```toml
# .anchor/probes.toml
[http.serde-max-stable-version]
url = "https://crates.io/api/v1/crates/serde"
select = "$.crate.max_stable_version"

# .anchor/anchors.toml
[[anchor]]
key   = "serde-max-stable-version"
probe = "serde-max-stable-version"
shape = "value"
```

**The key is the name, never the URL.** `AnchorKey` admits at most 64 characters
(`check_key`), and a real URL with a query string passes that without trying — the
plan this came from said 128 and was wrong, which is D-3. But the length is the
smaller half of the reason: a key is what a person types at a terminal and writes
in a note's frontmatter, so it should be a name. The URL has no length limit where
it actually lives, in the declaration.

The name comes from `--as`, or is derived from the URL's last path segment and the
selector's last field. Derivation is a convenience and is allowed to be wrong; it is
not allowed to be *silently* wrong, so a name already pointing at a different URL is
an error naming both, not a second declaration and not an overwrite. Re-routing an
existing name is a criteria change and goes through `revise` / `accept --criteria`,
which is CLAUDE.md §2's rule about this file, applied to the one verb that writes it.
Re-declaring the *same* URL under the same name is idempotent.

## Why the transport reads the repository instead of being handed a map

`Http` is built once, when the process assembles its `Runtime`. `gmr anchor` then
writes a new probe and immediately opens an anchor with it — so a map captured at
assembly time is stale by exactly the probe the user just asked for, and the first
attempt fails with "no `http` probe named ...". That is not a race; it is the
ordinary path.

So the battery takes an `Asks` lookup rather than a `BTreeMap`, and this domain
implements it by reading `.anchor/probes.toml`. The declarations have one home and
it is the file — nothing caches a second copy that can be older than it. The cost is
a small TOML read per resolve, which is paid only by http probes and only when one
is used.

## `obs` is not in the declaration, because it is not a choice

An http probe always reports one field, under the name the transport exports as
`VALUE`, with the schema it exports as `SCHEMA`. Writing `facts = ["value"]` into
the TOML would be a second copy of a fact the transport already decides, free to
drift the day the transport changes it. `http_obs()` builds it from those constants
instead, so the declaration carries only what a person actually chose: where to look
and what to pick out.

## When this changes, ask

Does a derived name start being allowed to collide? The whole value of deriving one
is that the failure mode is a refusal a person can read, not an anchor quietly
watching the wrong endpoint.

Does anything start putting the URL in the key? Then `check_key` is the next thing
that breaks, and after it every note's frontmatter and every command line.
