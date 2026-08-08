---
anchors:
  - key: doctrine::decisions
    probe: prose-map
    position: { file: CLAUDE.md, heading: "一、这十三条是 owner 定的，不要重新论证" }
    shape: fingerprint
  - key: doctrine::red-cards
    probe: prose-map
    position: { file: CLAUDE.md, heading: "四、红牌 —— 违反了不会有人发现的那些" }
    shape: fingerprint
---

# The criteria themselves

`CLAUDE.md` is the single source of the positions and the red cards. These two
anchors watch it directly.

## When the fingerprint changes, ask

There are only two possibilities: **the owner changed the criteria**, or **someone
changed what they should not have**. Observationally the two are identical and the
substrate cannot tell them apart — so it hands this section back to you, and you
say which it was.

The mechanism that rotted last time round was exactly this third link: AI writes an
argument → overturns the owner's decision → the argument goes into the document →
the next round reads the document and takes the argument for a criterion. These two
anchors watch that link.

This layer is **orthogonal to code granularity**. The anchors under `crates/` watch
whether some piece of code moved; these two watch whether the basis for judging
*whether that code should have moved* has moved. When the former changes, go look at
the code. When the latter changes, go re-read every memory.

## When red-cards reports missing, ask

The "四、红牌" section has been gone since `5f6b22d rewrite claude.md`, and this
anchor showed itself as having captured a baseline the whole time — because `file`
matched, `heading` did not, the probe fell back to the first heading in the file,
and the capture rule of the day did not look at `exact`, so it pinned that wrong
section as the baseline. That is why the two doctrine anchors had identical
fingerprints and both pointed at line 7.

It only started telling the truth once the rule order was fixed. Its `missing`
report now is **correct**, and the question to answer is: was the red-cards section
deliberately deleted, or was it moved? If deleted, close this anchor; if moved, fix
the heading. Until you answer, it will keep reporting — which is exactly what it
should do.
