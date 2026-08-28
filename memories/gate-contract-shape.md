---
about:
  - tools/gate.py#check_contract_shape_is_earned
  - tools/gate.py#contract_shape
  - tools/gate.py#contract_types
  - tools/gate.py#declaration_of
watch: [sig, logic]
---

# The contract's version has to be noticed, not just correct

`crates/gmr-runtime/src/contract.rs` is the whole of D-6's line: `Instructions`
in; `Warrant` (with `Holding`, `Knowledge`, `Blind`), `Grounding` (with
`Before`), `Verifiability` (with `Openness`), `Ref` and `Version` out. Membership
of that module is the marking — nothing was made private, because `Footing` and
the two `Kind` keys still have real callers in `doctor` and `render`. What
changed is that they are now *outside* a named boundary instead of merely absent
from a list.

A line is only worth drawing if crossing it is noticed, and adding a field is the
quiet way to cross it: nothing fails to compile, every test still passes, and a
consumer that matched exhaustively on last week's shape is broken by a diff that
never mentions the contract. So `SHAPE` is an earned hash in rule 5's sense —
over every input that can change what a caller sees, recomputed from the
declarations themselves so it cannot drift from them.

## Why this one is hand-recorded when [[eval-version]] is not

`EVALUATOR_VERSION` is computed in `build.rs`, and that is right for it: its job
is to be **correct**, always, with no chance for a human to forget. Copying that
here would defeat the purpose. A hash the build recomputes silently absorbs the
field you just added — the number changes, nothing fails, and the contract has
moved with no one told. That is the failure this check exists to prevent, so the
digest is written down by hand and the gate refuses until it matches.

Correctness is not the scarce thing on this side. **Friction is.**

## Two halves, because recording the digest is not the promise

The first half runs in any tree with no git: recompute, compare to `SHAPE`. It
catches the field you added this afternoon.

It does not catch updating `SHAPE` alone. So the second half reads
`contract.rs` at the latest release tag: a shape that moved since then without
`CONTRACT` moving with it is the case the check is named for — callers pin that
string to know what they may match on, so a shape that moves under it is a break
they were told did not happen. It skips when there is no tag and when the module
did not exist at that one, which is how it stays quiet on the commit that
introduced it.

## The shape is the declaration verbatim, deliberately over-wide

`declaration_of` captures the attributes, the head, and the braced body — so
`#[serde(tag = "holding")]` and `rename_all` are inside the hash, because they
decide the wire shape as surely as a field name does. It over-triggers: renaming
a field's type with no change to the JSON still fires. That is the cheap
direction to be wrong in, and `cargo fmt --check` runs earlier in `gate.sh`, so
whitespace cannot make it fire on its own. Zero comments in the clean zones
(§3) means prose cannot either.

`Version` has no braced body — it is a `string_newtype!`. Its entry is the
`admitted Version, ...` line **plus the named validator's body**, so narrowing
`check_nonempty_128` from 128 to 64 is a contract change that fires. It would
otherwise be the exact silent break D-3 found the plan had already made once,
in the other direction, by misreading `AnchorKey`'s bound.

## The list of contract types is not written here

`contract_types` reads the `pub use` lines out of `contract.rs` and resolves each
name to its declaration. There is deliberately no roster of these names in
`gate.py`: that would be the same second copy [[layers]] and the trait-roster
check were built to stop, one directory over — a list that goes on being checked
after it stops being true. Adding a type to the contract is one edit, and the
gate follows.

The cost is that a name the module re-exports but the gate cannot resolve is an
error rather than a shrug. That is the right direction: an unresolvable name
means the shape stopped being computed over part of the contract, which is
indistinguishable from the contract having no guard at all.

## When this changes, ask

Does someone move `SHAPE` into `build.rs` to stop having to update it by hand?
That is the whole check, deleted. Read the section above first.
