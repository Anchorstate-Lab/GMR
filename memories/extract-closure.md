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

By the same logic the shared files are read into the hash whole. **Touch one byte
of any of them — add a constant, add a test, delete a comment line — and all four
extractors swap versions, requiring one `gmr rebase --all`.** So anything that
touches them has to be compiled into the same commit, the same migration. This
hash would rather over-report than under-report.

## The shared set is default-in, and that is the whole point

`shared_files` lists `batteries/survey/src` and hashes **everything** in it,
minus `WAIVED`. It used to be a list of two filenames, and the failure mode of a
list is silence: `cache.rs` sat outside it while deciding what reached `collect`
and when an entry was fresh, and nobody noticed until somebody went looking.

Adding a file to that directory now changes all four versions with nobody having
listed it. Checked by adding one: the versions moved. Leaving a file out is a
sentence someone has to write, in `WAIVED`, next to the reason. Today those are
storage (`cache.rs`, `index.rs`, `sqlite.rs`, `testkit.rs`), a function proved
output-preserving (`narrow.rs`, see [[survey-narrow]]), and `lib.rs`, which
declares modules and re-exports.

A stale waiver is a hole nobody can see, so `shared_files` refuses to build when
one names a file that is gone. Checked by renaming `narrow.rs`: the build stops
and says which waiver went stale.

## Which file moves which version, measured

The algorithm here was recomputed outside the build and reproduced all four
hashes byte for byte, so this is a measurement rather than a reading of the code:

```
matching.rs  HASHED -> all four      cache.rs   WAIVED -> none
recipe.rs    HASHED -> all four      index.rs   WAIVED -> none
walk.rs      HASHED -> all four      lib.rs     WAIVED -> none
                                     narrow.rs  WAIVED -> none
                                     sqlite.rs  WAIVED -> none
                                     testkit.rs WAIVED -> none
```

The consequence worth writing down: **`look` lives in `recipe.rs`**, so the shape
of the query — where candidates come from, in what order they are fetched — is
inside all four closures. Changing it is a `rebase --all` in every repository
using these probes, whether or not a single answer moves. Anything that will
have to change when the index lands belongs in a waived file *before* then, and
the switchover has to be one commit.

Use `gmr probes list` to read the four versions; do not write a second
implementation of this hash to check them, because a copy that drifts would
answer confidently and wrongly about exactly the thing this file exists to make
honest.

**`eligible` is the reason `cache.rs` can stay out.** Which files reach `collect`
is now declared by each extractor and hashed with it (see [[survey-recipe]]);
what is left in `cache.rs` is freshness, and freshness cannot change a pure
`collect`'s answer — same bytes, same candidates. The day `collect` stops being
pure, that waiver is wrong.
