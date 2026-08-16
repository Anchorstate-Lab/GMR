---
about:
  - batteries/atlas/src/render.rs#render
  - batteries/atlas/src/graph.rs#check
watch: [sig, logic]
---

# A page that is opened from a path has no second chance to fetch anything

The output is opened as a `file://` URL. There is no origin, no server and often
no network, so every library, the stylesheet, the script and the caller's logo
are inlined rather than linked — the logo as a `data:` URI, which is a reference
that resolves to bytes already in the file rather than a fetch. A CDN reference
would not fail loudly here; it would produce a page that looks like it loaded and
simply has no graph on it, which is the failure shape this project refuses
everywhere else. The test does not forbid `<img`, it forbids a `src` that points
anywhere a file:// open cannot follow.

The data is embedded with every `<` rewritten to its JSON escape. `<` can only
occur inside a JSON string, so the rewrite is lossless, and without it any memory
whose text quotes a closing script tag would end the data element early. That is
not hypothetical for a corpus that documents HTML-adjacent code, and the damage
is invisible from this side: the file is written, the exit code is zero, and the
page is broken only when someone opens it.

`check` runs before a single byte is produced, for the same reason. A duplicate
id or an edge naming a node that is not in the graph makes the layout throw in
the browser, where nothing the CLI can see would ever hear about it. Refusing at
generation time turns a page that fails in front of a person into an error in
front of the person who could fix it.

## When this changes, ask

Is anything new being written into the page that came from outside — a title, a
label, a caption? Then ask which of the two escapes it needs: text going into
markup needs the HTML one, anything going into the data element needs the JSON
one. Getting it wrong is not a crash, it is a page that renders a little wrong or
not at all.

Did an asset move from inlined to referenced to keep the file small? Size is not
the constraint being defended here; being openable with nothing else present is.
