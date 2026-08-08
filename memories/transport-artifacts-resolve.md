---
about: batteries/transport/src/shell/artifact.rs#resolve
watch: [sig, logic]
---

# A mismatched artifact is an unnameable rule, not an old version

`resolve` refuses on any mismatch — wrong schema, a manifest hash that
disagrees with the directory it is stored under, an entrypoint missing from
its own file list, a file whose content hash disagrees with the manifest —
rather than trying to run with whatever is there anyway. The reasoning is
in the return type, not just the checks: an artifact whose stored content
disagrees with what it claims to be is not "the previous version of this
probe", because we cannot even say what derivation rule it is standing for
anymore. Running it would mean executing bytes under a name that no longer
names them.

## When this changes, ask

Does a new failure mode here get treated as "close enough, run it anyway"?
Any mismatch between the manifest's claims and the artifact directory's
actual bytes has to stay a hard refusal — that is the whole point of
verifying byte-for-byte instead of trusting the stored version string.
