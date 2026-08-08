---
about: crates/gmr-core/src/probe.rs#Verifiability
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

## What each variant is saying

```
Closed   complete. What ProbeName resolves to is what actually runs
Open     something outside the hash can change the answer — an interpreter,
         the host environment, an implementation living somewhere else
```

`Open` is not "not done yet", it is a fact that has to be stated. Probes that
shell out to read `$HOME` or call cargo are born Open, and the transport layer
downgrades them to Open, which is what they deserve.
