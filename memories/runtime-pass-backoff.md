---
about: crates/gmr-runtime/src/pass.rs#pass
watch: [sig, logic]
---

# Our failures and the world's failures do not share a backoff curve

`pass` backs off `ReasonClass::Unevaluable` straight to the policy's
backoff cap, rather than letting it climb the normal attempt-based curve
`Disposition::Backoff { after_secs }` uses for other reasons. A broken
expression (a rule that cannot evaluate) will fail exactly the same way on
retry number two as it will on retry number ten thousand — nothing about
waiting longer makes it more likely to succeed, so ramping the backoff up
gradually the way transient world-failures deserve would only mean
spamming the log with identical failures for longer before finally hitting
the cap. Going straight to the cap gets there without the spam.

## When this changes, ask

Does the new reason class represent something that could plausibly resolve
on its own if retried (a network blip, a busy database) or something that
is deterministically broken until a person fixes it (a bad expression, a
missing declaration)? Only the first kind deserves the gradual
attempt-based backoff; the second belongs with `Unevaluable`.
