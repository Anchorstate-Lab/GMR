---
about:
  - crates/gmr-runtime/src/read.rs#Grounding
  - crates/gmr-runtime/src/read.rs#Before
  - crates/gmr-runtime/src/memory.rs#ground
watch: [sig, logic]
---

# One complete answer, because five `Option`s could contradict each other and did

`MemoryView` used to carry `current_version` / `rewritten` / `content` /
`content_at_bind` / `retrievable` / `unavailable` side by side. Nothing
stopped them disagreeing, and they did: a record that was not UTF-8 came
back with `rewritten: true`, `content: None` and `unavailable: Some(..)`
all at once, and a reader had no way to decide what that meant. `Grounding`
is one value that is always exactly one of the things that can be true.

## The three ways to have no content are three different people's problem

```
Gone          the provider answered: no such record      the world's answer
Unreachable   we could not get an answer                 our failure
NoProvider    no provider by that name is registered     an assembly fault
```

That split is [[layers]]'s `NotFound` / `ProbeError` line drawn one layer
over, and it is load-bearing rather than decorative: what a reader should
do differs in each case, and so does whether CI should go red. `Gone` and
`NoProvider` are things the person holding this repository can fix — an
unbind, a rebuild with that feature on. `Unreachable` is somebody else's
service having a bad minute, and a build that fails on it fails for reasons
its owner cannot act on.

`Unreachable` carries a typed `code` beside its human `why`, the same shape
`ProbeError` and `ContentError` already use. The sentence is for a person;
the code is for anything deciding what to do.

## `Before` distinguishes what the provider cannot do from what it did not keep

```
Retrieved      here is the bound version
NotRetained    this provider has history, and that version is not in it
NoHistory      this provider has no history at all
Unreachable    we could not ask
```

`NoHistory` is settled by asking `history()` once at construction — it is a
static fact about the backend, not an answer smuggled back through a call
that should never have been made (see [[provider-claude-memory-history]]
for how the old shape hid exactly this). `NotRetained` is about **one
binding**: mem0 lets a memory be written with an expiry, and Zep closes an
edge rather than keeping every version forever, so a store that fully
implements `History` still loses individual versions. Collapsing the two
sends the reader to fix the backend when the truth is about one record.

`Rewritten` keeps its `before` rather than degrading to `Unreachable` when
history cannot be reached. The rewrite is a fact we already established
from the current version; losing it because the *second* question failed
would throw away the actionable half of the answer.

## Bytes, not `String`

Content is `Vec<u8>` all the way through the runtime. Deciding what to do
about bytes that are not text is a rendering question, and pushing it down
here is what created the contradictory triple above. The JSON surface
serialises lossily and the terminal renderer says so when it happens; both
are render-layer choices made at the render layer.

## When this changes, ask

Does a new variant describe something the reader would act on differently,
or is it a detail of one that already exists? And can whoever receives it
tell, without asking anyone, whether it is theirs to fix — that question is
the whole reason these are separate rather than one `unavailable: String`.
