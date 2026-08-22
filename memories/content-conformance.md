---
about:
  - crates/gmr-content/src/testkit.rs#Corpus
  - crates/gmr-content/src/testkit.rs#Listing
  - crates/gmr-content/src/testkit.rs#conforms
  - crates/gmr-content/src/testkit.rs#lists
  - crates/gmr-content/src/testkit.rs#retains
  - crates/gmr-content/src/testkit.rs#a_store_out_of_reach_never_reads_as_an_absence
  - batteries/provider/src/local_file.rs#read
watch: [sig, logic]
---

# One suite per capability, each small for the same reason

A capability a store lacks is a trait it does not implement — `History`,
`MemorySource` — so "does this backend handle not having one" is not a
question a test can be asked. **The compiler settles whether a store has a
capability. It settles nothing about whether that capability, once claimed,
holds up**, and there is one suite per capability for exactly that gap:
`conforms` for `ContentProvider`, `lists` for `MemorySource`, `retains` for
`History`.

Each is entered by name rather than skipped when a trait is absent. A suite
that quietly passes because the capability was not wired is the failure this
whole layer exists to prevent, so the caller names which store claims what,
and `retains` refuses outright a store whose `history()` is `None`.

## `conforms` — the meaning of two answers, which no signature carries

- **A version tracks content and nothing else.** Same bytes ⇒ same version,
  different bytes ⇒ different version. Get it wrong one way and every
  untouched record reads as rewritten; wrong the other way and a memory that
  changed is never handed back — the quietest failure this system has.
- **`Ok(None)` is the world's answer, never ours.** A store that will not
  answer must stay `Err`, which D6 keeps out of every exit code. Confusing
  the two makes `doctor` print a screenful of bindings to delete that are
  all still there.

Neither is expressible in a type. Everything that *is* expressible in a type
was left to the type.

## `lists` — a listing is an offer, and an offer has to be honourable

Three laws, and each one names a way a listing can be worse than no listing:

- **The listing addresses the store it came from.** `MemorySource::provider`
  and `ContentProvider::provider` must agree, because a binding is looked up
  by the provider name its reference carries. Disagree, and every record the
  listing offers binds to a store that is never asked.
- **Everything offered can be fetched** — every record in the listing, not
  just one the suite planted. `gmr memories` exists so a reference can be
  found without guessing one, so a listing naming records the store cannot
  answer for offers nothing but bindings `doctor` will say to delete.
- **A listed version is the one `fetch` computes.** `sync` stamps a binding
  with the version the listing gave; `read` compares it against the version
  the provider computes. Two ways of arriving at a version means one store
  state where they disagree, and there every record reports as rewritten
  with a bound version nothing can retrieve.

The third is general, not a property of files: it is the same fact the note
directory needs and the same one a network store can get wrong.

## `retains` — a version that never existed is an answer, not a failure

One law, because only one generalises: asking for a version this store never
issued must be `Ok(None)`. A version that has genuinely fallen out of a log
is the world's answer and renders as *the bound version was not kept*; a
store that will not answer is our failure. Reported as the second, every
consolidated-away version turns a build red that nobody holding the
repository can fix.

The obvious companion — *the version a `fetch` just returned is retrievable*
— is **not** a law, because it is false for git: `hash-object` versions a
working-tree file whose blob `cat-file` cannot find. One law that holds
everywhere beats two where one convicts a conforming store.

## What was ruled out, and why it stays ruled out

A `gate.py` rule forbidding provider names in the domain was considered and
dropped. The domain legitimately names providers today — `provider_warning`
twice, and five `default_value = "git"` in the CLI — so the rule would have
shipped with a whitelist, and a rule that needs exceptions on its first day
is one the next person routes around rather than argues with.

A test that `fetch_at` is never *called* on a provider with no history was
also dropped: `history()` returns `Option<&dyn History>`, so `None` leaves
nothing to call. It was a test of the compiler. `retains` is not that test —
it asks what `fetch_at` answers, which no signature carries.

The rule this leaves behind is worth stating: **what a type can express, a
check should not.** A check is a wall — cheap to route around, and silent
once someone does. A type is a more accurate word: changing it means editing
the definition, which is a visible decision with a diff and an anchor
watching it.

## It found a real one on its first run

`local_file::read` returned `Ok(None)` for any path under a root that does
not exist. For git that is unreachable — the CLI canonicalises the repo root
and fails first. For claude-code it is the **ordinary case**: its memory
directory is only created once a session has written there, so any project
without one answered "gone" for every binding it held, and `doctor` told the
reader to delete all of them.

Nothing else would have caught it. The provider's own tests all wrote a file
first, which is exactly the shape of test that cannot see this.

## mem0 runs these live, because a corpus means writing

`Corpus::holding` asks a backend to put given bytes somewhere and name them.
The mem0 battery cannot: its seam offers `get` and nothing else, which is
the guarantee it exists to keep ([[provider-mem0]]). So mem0's arm is
`tests/mem0_live.rs`, `#[ignore]`, driven by the same environment as the
wire-shape canary — and it carries the out-of-reach check explicitly,
because that is the half a fake could never establish.

The self-hosted arm needs no credentials at all: mem0's own server boots
against a dummy key, since its read path never touches an embedder. Whether
that becomes a scheduled job is a separate decision from this suite existing.

## When this changes, ask

Does a suite grow a law? Ask first whether a type could carry it, then
whether it holds for every store rather than the one in front of you — the
retrievable-version law failed the second question, and it looked obvious.

Does a capability get a trait without getting a suite? Then having it and
implementing it correctly are the same question again, and the answer is
whatever the first backend happened to do.

Does `Corpus` grow a method a backend cannot honestly implement? mem0
already cannot implement `holding`, and the honest answer was to run its
arm live rather than to fake a corpus — a fake corpus would have made the
suite green for every backend and meaningless for the remote ones.
