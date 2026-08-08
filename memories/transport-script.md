---
about:
  - batteries/transport/src/script.rs#Script
  - batteries/transport/src/script.rs#the_identity_is_the_file_and_one_byte_moves_it
  - batteries/transport/src/script.rs#moving_a_line_between_files_moves_the_version
  - batteries/transport/src/script.rs#the_environment_is_inherited
watch: [sig, logic]
---

# `Script`'s identity is the file, hashed — and its openness is honest, not lazy

A script's version comes from hashing its own content at call time
(`closure::of_path`, see [[transport-closure]]) — the script itself never
gets to declare what it is, and one changed byte changes the version. When
the entry is a directory, every file under it folds into the same hash by
path and bytes, so moving a line between two files in the closure still
moves the version even though no single file's bytes look different in
isolation.

`Script` always reports `Verifiability::Open`, never `Closed`, and that is
a deliberate admission rather than a missing feature: the interpreter that
actually reads the script (`sh`, `python3`, whatever `#!` names) is not
part of the hash, and the child process inherits this process's environment
rather than running with it cleared. Clearing the environment would only
break the script's ability to find its own interpreter on `$PATH`; the
honest move is to inherit it and say plainly that the closure is open,
instead of quietly claiming a closed guarantee this transport cannot back.

## When this changes, ask

Does the new behavior let `Script` claim `Verifiability::Closed` for some
case? It cannot, as long as the interpreter and the inherited environment
stay outside the hash — either fold them into the version closure first, or
leave the verifiability as `Open`.
