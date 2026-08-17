---
about:
  - crates/gmr-content/src/testkit.rs#Corpus
  - crates/gmr-content/src/testkit.rs#conforms
  - crates/gmr-content/src/testkit.rs#a_store_out_of_reach_never_reads_as_an_absence
  - batteries/provider/src/local_file.rs#read
watch: [sig, logic]
---

# Two checks, because everything else a store owes is the compiler's job

This suite is deliberately small, and the size is the point. A capability a
store lacks is a trait it does not implement — `History`, `Declaring`,
`MemorySource` — so "does this backend handle not having one" is not a
question a test can be asked. What is left is the meaning of two answers,
which no signature carries:

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

## What was ruled out, and why it stays ruled out

A `gate.py` rule forbidding provider names in the domain was considered and
dropped. The domain legitimately names providers today — `provider_warning`
twice, and five `default_value = "git"` in the CLI — so the rule would have
shipped with a whitelist, and a rule that needs exceptions on its first day
is one the next person routes around rather than argues with.

The test that `fetch_at` is never called on a provider with no history was
also dropped: `history()` returns `Option<&dyn History>`, so `None` leaves
nothing to call. It was a test of the compiler.

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

Does a third invariant get added? Ask first whether a type could carry it.
The two here survived that question; nothing else that was proposed did.

Does `Corpus` grow a method a backend cannot honestly implement? mem0
already cannot implement `holding`, and the honest answer was to run its
arm live rather than to fake a corpus — a fake corpus would have made the
suite green for every backend and meaningless for the remote ones.
