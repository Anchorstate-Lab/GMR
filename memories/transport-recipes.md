---
about:
  - batteries/transport/src/recipes.rs#Recipes
watch: [sig, logic]
---

# A declaration a caller cannot write in Rust is a declaration nobody outside this repo can make

`Asks` is a Rust trait, and until G1.5 its only implementation lived in the coding
domain and read `.anchor/probes.toml`. That is fine for a CLI in a git repository
and impossible for anyone else: a Node or Python caller cannot implement a Rust
trait, and may have no repository to put a TOML file in. The SDK would have shipped
with six verbs and no way to configure a probe.

`Recipes` is that missing entrance — **data, not a trait**. Three fields, each
gated by the feature that defines its transport, each a `ProbeName → Ask` map, and
one `Asks` implementation per family so all three transports read the same value.
A build without `sql` has no `sql` field to fill in, which is what a feature gate
is *for*: a declaration this binary cannot honour is not a runtime error to
discover, it is a field that is not there.

## The `Ask` types are the declaration, in the file and on the wire

There used to be an `HttpDecl` next to `http::Ask`, a `FileDecl` next to
`file::Ask`, a `SqlDecl` next to `sql::Ask`, each with an `ask()` that copied
field for field. Two structs describing one declaration is two shapes that drift:
add a field to one and it is silently absent from the other, and the version the
probe earns is computed from whichever copy the transport happens to hold.

So `Ask` grew `Serialize` + `Deserialize` and the `Decl` types went away.
`.anchor/probes.toml` and a JSON body handed in by an SDK now deserialise into the
same struct, which is why
[[cli-fetched-facts]] can keep writing TOML while an SDK sends none.
`sql`'s source moved with them: `url` / `url_from_env` as two optional strings
became `source = { given = … }` / `source = { from_env = … }`, which is the shape
`Source` already had in Rust and the shape `headers` already had in the file. Two
optional fields can both be set or both be empty and need a runtime check to say
so; an enum cannot.

## What a recipe may carry

`Source::FromEnv` and `Header::FromEnv` serialise **the variable's name and never
its value**, and this is not a convenience to be relaxed. A recipe is data now: it
travels over a wire, sits in a file, gets logged. Resolving the variable at
serialisation time would put a password into every one of those, including the
append-only journal, where nothing can take it back out. The variable is read once,
at the moment of the call, and what it holds never becomes part of anything that
is stored. A test asserts the serialised form carries `from_env` and no `given`.

A recipe earns the same [[probe-Derivation]] whether it arrived as TOML, as JSON,
or as a Rust literal, because `version()` hashes the declaration and the
declaration is now one type. Were they two, an observation made through one would
be `Incomparable` with an observation made through the other, for no reason a
person could see.

## When this changes, ask

Does something make `Recipes` reachable without its feature? Then a caller can
declare a `sql` probe to a binary with no sql transport, and the failure moves
from "that field does not exist" to an observation that never comes back.

Does a `FromEnv` ever get resolved before serialisation — for a diagnostic, for a
cache key, to make an error message clearer? That is how the credential leaves the
process, and the journal keeps whatever reaches it.
