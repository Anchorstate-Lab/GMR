---
about: domains/coding/cli/src/skill.rs#stale
watch: [sig, logic]
---

# The one contract with agents that upgrades silently leave behind

`SKILL.md` is compiled into the binary with `include_str!` and installed on
disk by `gmr init` through `write_new`, which **skips a file that already
exists**. That is right for a file a person may have edited, and it means
an upgraded binary leaves the old text in place forever: agents keep
reading a contract this build no longer honours, and nothing anywhere
notices. It is the same failure shape as a comment outliving the code it
described, one layer out.

So `stale` compares what is installed against what is compiled in — both
the project copy and the global one, since either may exist and either may
be the one an agent actually reads. A file that is absent is not stale:
plenty of repositories deliberately have no copy, and reporting that would
make the signal noise.

It reports rather than repairs. Rewriting a file under
`.claude/skills/` would be writing into another product's directory, which
is the guarantee [[provider-claude-memory]] exists to keep on the reading
side; doing it here on the writing side would be worse. `doctor` prints the
path and the fix.

Its first run against this repository found a real one: `assets/SKILL.md`
had been edited two commits earlier and the installed copy had not moved.

## When this changes, ask

Does it start writing the file instead of reporting it? And does an absent
copy start counting as stale — that turns every repository that never ran
`gmr init` into a red build for a file it chose not to have.
