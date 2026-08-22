---
about:
  - domains/coding/cli/src/verbs/memories.rs#run
  - domains/coding/cli/src/stores.rs#silent
  - domains/coding/cli/src/stores.rs#listing
watch: [sig, logic]
---

# A store that cannot be listed is named, not filtered out

`listing` and `silent` partition the registered stores; both are reported.
A store with no `MemorySource` gets a line saying it is registered and
cannot list what it holds — it is not empty, not broken, and not absent.

Filtering it away was the earlier behaviour and it produced the exact
failure [[content-discovery]] was written to prevent one layer down: a
reader who cannot see a store cannot tell "this store holds nothing" from
"nothing here can enumerate this store", and the repair for those is not
the same. The trait already refuses to answer "I have none"; the CLI must
not reintroduce that answer by omission.

## The wording does not change with `--provider`

`gmr memories --provider <a store that cannot list>` prints the same line
`gmr memories` prints for it, and exits 0. Naming a store is not a
different question from listing everything, so it must not get a different
answer — and "cannot be enumerated" is an answer, not a failure to produce
one.

`--provider <a name nothing registered>` is still an error, and says which
names are registered. That one is a fault the reader can fix: a typo, or a
feature this build does not have.

## When this changes, ask

Does a new store-shaped condition get handled by leaving it out of a
listing? Every condition a reader would act on differently needs its own
line. Silence is indistinguishable from every other silence.
