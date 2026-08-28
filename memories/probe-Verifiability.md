---
about:
  - crates/gmr-core/src/probe.rs#Verifiability
  - crates/gmr-core/src/probe.rs#Openness
  - crates/gmr-core/src/probe.rs#an_entry_written_before_the_open_surface_existed_stays_unknown
watch: [sig, logic]
---

# Open is not a failure, it is the sentence that has to be said out loud

`Verifiability` says whether `Derivation::version` has closed over everything that
can change the output.

**Open is not a defect.** A probe that shells out to ask cargo, or reads `$HOME`,
cannot close — it is reporting facts about the host. The problem was never "there
are open probes", it is "there are open probes and nobody knows". The entire
reason this enum exists is to force that sentence to be said and to enter the log.

## When this changes, ask

Someone wants to add a "counts as closed really" bypass for an open probe → that
swallows the sentence. If you genuinely want it closed, go and eliminate the
external input; do not change the label.

A new transport arrives → what does it fail to close over? Answering "nothing"
makes it `Closed` and that had better be true. Answering `Open` without naming
the surface is the bare `Open` this note replaced, and the only spelling for
"we did not record it" is `Unknown`, which is reserved for rows written before
anyone could.

## What each variant is saying

```
Closed   complete. What ProbeName resolves to is what actually runs
Open{over}  something outside the hash can change the answer, and `over` names
            what: host_env · interpreter · network · clock · implementation
```

`Open` is not "not done yet", it is a fact that has to be stated. Probes that
shell out to read `$HOME` or call cargo are born Open, and the transport layer
downgrades them to Open, which is what they deserve.

## `Open` carries what it did not close over, and the reason is timing

A bare `Open` says the sentence but not which one. "An interpreter resolved from
`PATH`" and "a remote system that may be down" are both open and are not the
same risk, and a grading that wanted to tell them apart later could not: the
distinction was never written down.

It is on the variant rather than beside it because *what is open* is a
sub-classification of the same axis, not a second one — the same reason
`Blind { why }` nests instead of sitting beside `Knowledge`. `Closed { over }`
is unrepresentable, which is the invariant in the type rather than in this
paragraph.

Half of it already existed and was being thrown away. `.anchor/probes.toml`
declares `env_from_host = ["HOME", "PATH"]`, and `Shell::resolve` already reads
`manifest.env` to decide `Closed` vs `Open` — it knew the reason and recorded
only the verdict. Now it records the reason.

**The timing is the whole point.** A version hash commits to the state at the
moment it is computed, and the same is true of a grade: entries written before
the surface existed can only ever be *blessed*, never re-graded. So the field
goes in before the probe families of M2 mint observations against live systems,
not after. Anything from before deserialises to `Unknown`, and a test says so.

`Verifiability::open([])` refuses the empty set and answers `Unknown` too: an
`Open` with nothing outside it is `Closed` spelled badly, and this note's rule
above is that there is no "counts as closed really" spelling.
