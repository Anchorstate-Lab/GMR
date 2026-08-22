---
about:
  - batteries/provider/src/claude_code.rs#ClaudeMemory
  - batteries/provider/src/claude_code.rs#new
  - batteries/provider/src/claude_code.rs#at
  - batteries/provider/src/claude_code.rs#memory_dir
  - batteries/provider/src/claude_code.rs#store
  - batteries/provider/src/claude_code.rs#list
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

## It lists by walking, and reading is the only thing that walk does

`list` is `local_file::walk` over the same directory `fetch` reads, keeping
only `.md`, versioning each record by the same content hash `fetch`
computes — so the law that a listed version equals the fetched one holds
because there is one derivation, not two that agree today.

The walk lives in `local_file` rather than here because `git` reads a
directory too, and the domain CLI's own `walk` over `memories/` is not
reachable from a battery — a battery that depended on a domain would be the
layering inverted, and "reuse" written down without a path to it becomes a
copy.

A directory that is not there is an `Err`, never an empty listing, for the
same reason `read` refuses it: this store's directory does not exist until
a session has written in it, and reporting "holds nothing" would say every
record in it was deleted.

Ids are paths relative to the memory directory, so a listing round-trips
through `fetch` even when the directory has grown subdirectories.

## When this changes, ask

Does the change make `ClaudeMemory` write anything back into Claude Code's
directory? A walk is a read; a listing that repaired, indexed or normalised
anything it found would cross the same line a write does. That crosses the one guarantee this type exists to keep. And if
a new test needs `GMR_CLAUDE_MEMORY_DIR`, does it also mutate/restore the
env var — a second test doing that concurrently breaks the single-threaded
assumption the `unsafe` block relies on.
