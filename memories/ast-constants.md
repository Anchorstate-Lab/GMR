---
about:
  - packs/coding/extract/src/lang.rs#RUST
  - packs/coding/extract/src/ast.rs#a_constant_is_a_coordinate_something_can_be_anchored_to
  - packs/coding/extract/src/ast.rs#a_constants_declared_type_is_part_of_its_shape
watch: [sig, logic]
---

# A constant is a thing worth anchoring, and for a while nothing could

The Rust table had no `const_item`, so `ast-map` could not see a Rust constant
at all. An anchor on one was not wrong, it was **permanently unanswerable**: the
coordinate reported `missing`, the anchor sat at `absent` forever, and the memory
bound to it could never be handed back no matter what happened to the constant.

`doctor` showed nothing. It reports an anchor with no memory and a memory with no
anchor; it cannot report a memory bound to a coordinate the probe is unable to
resolve, which looks identical to a memory about something that has not been
written yet. Four of them had accumulated in this repository, and they were not
minor ones — the probe wire protocol, the export format version, the shipped
skill document, and sync's default path. Every one is a constant where a silent
change is the whole danger.

## Why the value has to be the body

The first version of this only added the kinds, and the test caught that
`facts.body` came back as the hash of the empty string for every constant. A
`const_item` has no `body` field — its value lives in a `value` field — so
`logic` could never move. An anchor on `POSITION_ENV` would have reported its
type changing and stayed silent about `"GMR_POSITION"` becoming something else,
which is exactly backwards for a constant.

`body_fields` is per-language for that reason: what counts as "the implementation"
is a language's answer, not a universal one. Rust's is `body` or `value`; the
others declare only `body` today.

The declared type lands in `sig` because `shape_fields` already fetches `type`,
so a `u8` becoming a `u64` reads as a contract change, which it is.

## Only Rust, and that is a judgment rather than an omission

The same hole exists in the other three tables, and the node names were read out
of the grammars rather than guessed. Each one carries a hazard Rust does not:

- **Go** `var_spec` and **Python** `assignment` also match every local variable
  inside every function body. Adding them floods the candidate set, and a flooded
  set is reported as "this coordinate is too broad" — a sentence pointing away
  from the cause.
- **TS** `variable_declarator` is already how an arrow function gets its name, so
  `const f = () => {}` would become both a function and a constant, and `nth`
  would start counting different things.

Each needs a decision about top-level versus local that Rust did not force. They
are left alone until someone makes it, deliberately, rather than half-made here.

## When this changes, ask

Does a new kind match nodes that also appear inside function bodies? That is the
line between a coordinate someone can anchor and a roster nobody can read. And
does every kind that carries meaning in a field other than `body` have that field
in `body_fields`? If not, its `logic` axis is dead and says so by never firing —
the quietest way for an anchor to be useless.
