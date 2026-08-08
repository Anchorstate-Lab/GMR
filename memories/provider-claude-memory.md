---
about:
  - batteries/provider/src/claude_code.rs#ClaudeMemory
  - batteries/provider/src/claude_code.rs#new
  - batteries/provider/src/claude_code.rs#at
  - batteries/provider/src/claude_code.rs#memory_dir
  - batteries/provider/src/claude_code.rs#env_override_bypasses_the_directory_guess
watch: [sig, logic]
---

# This battery only ever reads Claude Code's memory directory, and its path is a guess

`ClaudeMemory` never writes into `~/.claude/projects/<mangled>/memory` —
that directory belongs to another product's data, and this battery only
observes it. `new` derives the mangled directory name from `project_root`
the same way `Git` derives its root, by replacing every `/` in the absolute
path with `-`. That mangling is Claude Code's own internal convention, not
a documented contract; it was verified empirically against a real
`~/.claude/projects/` tree, including a path whose basename already
contained a hyphen (`.../moltbook-001` → `-...-moltbook-001`), confirming
the substitution stays unambiguous going forward even though it is not
reversible.

Because that convention could change or the guess could be wrong on some
machine, `GMR_CLAUDE_MEMORY_DIR` overrides it outright — `memory_dir` checks
that env var first and never even canonicalizes `project_root` if it is
set. `at` is the test-only counterpart: it points straight at a directory
without going through the guess at all, and production code has no reason
to call it since `GMR_CLAUDE_MEMORY_DIR` already covers that need — which
is why `at` stays private. `env_override_bypasses_the_directory_guess`
mutates that same process-global env var with `unsafe { set_var }`, safe
only because it is the sole test in this module touching that variable and
the crate's tests run single-threaded with respect to it.

## When this changes, ask

Does the change make `ClaudeMemory` write anything back into Claude Code's
directory? That crosses the one guarantee this type exists to keep. And if
a new test needs `GMR_CLAUDE_MEMORY_DIR`, does it also mutate/restore the
env var — a second test doing that concurrently breaks the single-threaded
assumption the `unsafe` block relies on.
