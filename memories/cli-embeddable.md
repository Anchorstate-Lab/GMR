---
about:
  - console/cli/tests/embeddable.rs#the_four_things_only_the_binary_could_reach_are_reachable_from_outside_it
watch: [sig, logic]
---

# A domain that is only a binary is a domain nothing else can use

`coding-anchor` shipped as `[[bin]]` and nothing else, so every module in it —
`Catalog::load`, `stores::assembled`, `Subscriptions`, `shapes::of` — was
private to the `gmr` process. Anything else wanting this domain's judgement had
two options: reimplement several thousand lines, or wait for this crate to come
apart. That is what made it the block ahead of every other shell: an MCP server
and an SDK adapter are both *second front ends*, and there was no front to be
second to.

It is now `[lib] + [[bin]]`, and the cut is by what is true of a terminal rather
than by what looks like plumbing:

```
library   parsing onward -- dispatch, assembly, every verb
binary    building a tokio runtime by hand, and ExitCode
```

`main` keeps `shutdown_background()` for the reason [[cli-main-run]] gives, and
that reason is exactly why it cannot move: an embedder brings its own runtime
and decides for itself when to stop waiting on it. A library that detached
somebody else's threads would be answering a question it was not asked.

## The test is the part that lasts

Splitting is easy to do and easy to undo by accident — one `pub` dropped in a
tidy-up and a module is private again, with nothing failing, because the binary
still compiles. `tests/embeddable.rs` links against the library the way any
other consumer would and touches the four surfaces the split existed for. It
goes red the moment one of them stops being reachable, which no amount of prose
here would.

It is also the only coordinate this note watches. A whole-file anchor on
`lib.rs` was the obvious second one and is the wrong shape: a bare file is a
roster, so it reports `grew` on every module added and says nothing about
whether anything outside the binary can still call in. The test is the fact.

## When this changes, ask

Does something in the library reach for the process — `std::process::exit`, a
runtime it builds itself, `current_dir()`? Those are the binary's to decide, and
each one silently narrows the library back down to one caller.
