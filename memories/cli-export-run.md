---
about: console/cli/src/verbs/export.rs#run
watch: [logic]
---

# Status always goes to stderr, because stdout might be the export itself

When `--out` is omitted, `run` writes the JSONL export straight to stdout
— so stdout has to stay a clean stream of export lines with nothing else
mixed in. That is why both the summary line and the settings/queue caveat
go to `eprintln!` unconditionally, not just when `--json` is set: anything
printed to stdout here would corrupt a piped export (`gmr export | gmr
import`, or redirected to a file).

## When this changes, ask

Does a new message in `run` go to stdout instead of stderr? If stdout can
ever be the export payload, any such message would land inside the JSONL
stream and break replay.
