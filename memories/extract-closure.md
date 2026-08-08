---
about:
  - domains/coding/extract/build.rs#gmr_outcome_contract
  - domains/coding/extract/build.rs#locked_versions
watch: [sig, logic]
---

# A closure cannot depend on the thing it is there to guarantee

## A build script cannot link the crate it is building

So `gmr_outcome_contract()` writes `"gmr.outcome.v1"` by hand, kept in line with
`gmr_core::OUTCOME_CONTRACT` by a test in `lib.rs`. This is **a second copy made
knowingly**, and the only justification is that build time cannot reach the first
one — and the test that watches it landed at the same time, not "later".

## Cargo.lock is parsed by hand

`locked_versions` picks `[[package]]` apart itself rather than using a TOML
parser. Because the whole job of the closure is "every input that can change the
output enters the hash", and pulling in a parser makes the hash depend on that
parser's version — one upgrade and every probe version in the repository turns
over, without the result of a single observation having changed.

By the same logic `SHARED` (`matching.rs` · `walk.rs`) is read into the hash whole.
**Touch one byte of either file — add a constant, add a test, delete a comment
line — and all four extractors swap versions, requiring one `gmr rebase --all`.**
So anything that touches them has to be compiled into the same commit, the same
migration. This hash would rather over-report than under-report.
