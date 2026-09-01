---
about:
  - crates/gmr-store/tests/send.rs#opening_a_store_is_something_a_host_can_move_between_threads
  - crates/gmr-runtime/tests/send.rs#every_verb_a_host_can_call_is_a_future_a_host_can_spawn
watch: [sig, logic]
---

# A future nobody can spawn is a library nobody can host

Every IO path in this repository was written for one caller: a CLI that awaits
on the thread it started on. That caller never needs a future to be `Send`, and
`cargo test` never asked, so two of them quietly were not — and nothing could
have noticed, because being `Send` is a property no test asserts unless somebody
writes the assertion down.

The first host that is not the CLI needs it immediately. A Node addon hands the
work to a thread pool; so does `tokio::spawn`; so does any web handler. G2 found
both by compiling the binding, which is a terrible way to find out.

These two tests are the assertion. They never run anything — they hand each
future to a function that takes `F: Send` and let the compiler answer. A
`#[test]` body that only type-checks looks strange and is exactly right: the
failure this guards against is a compile error in somebody else's crate.

## The two that were not, and why

**`sqlx::raw_sql(sql).execute(conn)`.** Every other query in the store goes
through `sqlx::query(..)` under `#[async_trait]`, which boxes at a concrete
lifetime and so proves `Send` on the way past. The migration ladder used
`raw_sql` — needed, because a schema step is many statements — and calling
`.execute()` **on the query** rather than `.execute()` **on the connection**
leaves rustc trying to prove `Executor<'_>` for `&mut SqliteConnection` at every
lifetime at once, which the impl does not offer. `statements()` inverts it:
`conn.execute(raw_sql(sql))`. Same SQL, same connection, one direction the
compiler can follow.

**`stream::iter(keys)` in `ground`'s first phase.** A `slice::Iter<'_, AnchorKey>`
lived inside a `buffered` stream across every await in the batch. The stream now
walks an owned `Vec` — a clone of a handful of short keys, once per call, which
buys a future the host can move. That is a clone with a semantics: the stream
owns the list it walks.

Both are the same shape of bug: a borrow that outlives an await inside an opaque
future, invisible until somebody demands `Send`.

## When this changes, ask

Does a new `pub async fn` join the surface a host calls? Add it to
[[node-sdk]]'s verbs and to the runtime assertion in the same commit, or it is
`Send` by luck until the day it is not. `sample` arrived that way and went into
both.

Does a query start being written `query.execute(conn)` again in the store? It
compiles, it passes every test, and it takes the whole crate off the list of
things a threaded host can use.
