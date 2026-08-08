---
about: crates/gmr-runtime/tests/state_machine.rs#an_anchor_that_declares_no_rule_still_records_that_the_world_moved
watch: [logic]
---

# An empty transition table is a legal declaration, not a gap to fill

Opening an anchor with no rules at all is not an omission the substrate
should compensate for — it is a legitimate way to say "keep the record,
interpret nothing." The substrate must not invent a default status
vocabulary to fill that silence: with zero rules, every observation that
moves the underlying facts still gets a full journal entry
(`Observed::Unchanged`, not compacted to `Still`) precisely because the
facts moved even though no rule matched to change the judged state.

## When this changes, ask

Does a change introduce a fallback status or synthesized rule for anchors
with an empty transition table? That would mean the substrate is
supplying interpretation the domain explicitly declined to provide.
