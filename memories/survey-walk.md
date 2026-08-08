---
about: batteries/survey/src/walk.rs#visit
watch: [sig, logic]
---

# Walking a directory must sort, and must not walk into build output

Sorting is not cosmetic: scan the same untouched tree twice and the candidate
order has to be identical. If it is not, the "take the greatest hit vector" in
`report()` picks a different candidate on ties, and the anchor has changed subject
— while `nth` is still 0 and nobody changed anything.

Skipping dot-directories, `target` and `node_modules`: an extractor that walks in
starts reporting build output as if it were the repository. The roster suddenly
grows by thousands of candidates, `MAX_BYTES` catches it, and the error reported
says "this coordinate is too wide" — a sentence pointing in the wrong direction.
