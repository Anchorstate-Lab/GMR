---
about:
  - dist/npm/index.d.ts
  - dist/npm/index.js
  - console/node/test/verbs.mjs
watch: [grew, shrank, roll]
---

# The surface that leaves the repository is watched by its members, not its symbols

Three files carry the binding outward: `dist/npm/index.d.ts` declares the shapes a
caller may match on, `dist/npm/index.js` loads the platform addon and re-exports
`CONTRACT`, and `console/node/test/verbs.mjs` walks the discriminants against real
output.

None of the three has a symbol an extractor can address. A TypeScript declaration
file is declarations; a test's identity is the string in `test("...")`, and a string
is not a name. So these are member rosters — `grew` · `shrank` · `roll` — and the
fine-grained axes ([[layers]] calls them the other layer) never light up here.

`watch:` is checked against **each** anchor's rule table, so a note bound to a
roster and to a `sig`/`logic` anchor has no axis set that satisfies both. That is
why this is a second note and not more `about:` lines on [[node-sdk]].

## What each axis asks of this surface

```
grew     a declaration appeared      does [[node-sdk]]'s verb list still name them all
shrank   one went                    who was matching on it
roll     the members changed         a rename passes both gate checks and breaks every caller
```

`grew` on the typings arrives whenever the contract gains a type.
`check_contract_shape_is_earned` and `check_typed_surface_names_the_contract` between
them make it impossible for a shape to move without somebody editing the declaration
file — and neither of them asks whether the prose describing that surface still
matches it. This anchor is where that question gets asked.

## When this changes, ask

Did a member arrive that no verb in [[node-sdk]] mentions? Then either the roster is
ahead of the prose, or something is exported that nothing calls.

Did `verbs.mjs` shrink? A discriminant walk that stops walking one discriminant is
the failure that both gate checks are blind to by construction.
