---
about: batteries/provider/src/claude_code.rs#fetch_at
watch: [sig, logic]
---

# No history is a legitimate answer here, not a missing feature

`fetch_at` always returns `Ok(None)` because memory files carry no version
history of their own — there is no older copy to retrieve, ever. That is
exactly the case `MemoryLens` already models as `retrievable: Some(false)`,
so returning `None` here is reporting a true fact about this provider, not
standing in for an unimplemented method.

## When this changes, ask

Did Claude Code's memory files gain some form of history this provider
could actually retrieve? Only then does `fetch_at` deserve real logic —
otherwise a change here would turn a true "no history" into a false
"I couldn't find it."
