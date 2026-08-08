---
about: crates/gmr-core/src/addr.rs#NewtypeError
---

# A validation failure has to name which newtype it came from

A `string_newtype!` validator rejected a value. `type_name` is there so the
**caller wrapping it** can tell which newtype failed without parsing the prose in
`reason`.

This was bought by commit 15472be: `try_new` used to return `String`, so callers
either passed the whole sentence through or matched on substrings. The entire
point of a structured error is to stop the layer above from parsing the layer
below's prose.

## When this changes, ask

Has anything started reading `reason` programmatically? If code is matching on
its contents, what is missing here is a real enum and the field should be
promoted.

The test `try_new_failure_names_the_newtype_it_came_from` pins exactly this: a
caller can match out **which newtype failed** without parsing the sentence.
