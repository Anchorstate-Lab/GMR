---
about:
  - batteries/atlas/src/graph.rs#Tone
  - batteries/atlas/src/graph.rs#Node
  - batteries/atlas/src/graph.rs#Kind
watch: [sig, logic]
---

# The battery has its own words so that no domain's words have to fit in it

This battery could have taken `AnchorView` directly. It does not, and the reason
is rule 4: colouring by state would mean reading `status`, and `status` belongs
to whichever domain is calling. A battery that knows `drifted` is a battery only
one domain can use.

So the input vocabulary is this layer's own. `Tone` is presentation severity —
how loudly something is asking to be looked at — and the caller decides which of
its states earns which level. `badge` carries the caller's own word through
untouched, echoed and never branched on. Between them, a domain with thirty
statuses and a domain with three both fit, and neither has to rename anything.

Four levels is not a compromise forced by the enum. Ten distinguishable semantic
colours do not exist for a reader; the full vocabulary lives in text — badge,
list, filter chips — where it can actually be read. Colour is for *how much*,
words are for *what*.

`Kind` and `EdgeKind` are a different matter and are allowed here: anchors,
memories, bindings and links are shapes the substrate has for every domain, not
vocabulary any one domain chose.

`under` is an ancestry, not a path. It is a list of labels because splitting a
string on `/` would be this layer deciding that coordinates are file paths —
true for the coding domain and for nothing else it is supposed to serve. The
caller says what a node hangs under and this layer draws however many levels it
was given; a domain whose coordinates have no hierarchy passes one level, or
none. What the tree does with that ancestry — folding a chain of only-children
into one row, carrying the worst descendant tone up so a collapsed branch cannot
hide an alarm — is drawing, and stays here.

There is deliberately no legend field. The page collects the `(badge, tone)`
pairs the nodes actually carry, so the legend is a projection of the data rather
than a second copy of it — the same reason `reason` is derived and not stored
beside the entry (see [[journal-reason]]). A declared legend could disagree with
what is drawn; a derived one cannot.

## When this changes, ask

Does a new field carry meaning this layer would have to interpret rather than
draw? The test is whether a second domain, with entirely different words for its
states, could fill that field without explaining itself. If not, the decision
belongs on the caller's side and only its outcome belongs here.
