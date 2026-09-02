# A menu, an allergen, and no code anywhere

gmr's own repository supervises Rust, so it is easy to mistake the tool for a
coding tool. This example is the counter-proof you can run:

```sh
sh examples/menu/walkthrough.sh        # narrated
sh examples/menu/walkthrough.sh --ci   # same steps, asserted, no pauses
```

It plays out in a throwaway directory containing exactly one fact source —
`menu.json` — and walks both loops on it.

## What happens

**The memory loop.** One command declares the fact, writes the memory, and
binds them:

```sh
gmr anchor 'file://menu.json#$.items.2.ingredients' --as kung-pao-ingredients \
  -m 'The order page must show the nut-allergy warning for Kung Pao Chicken: …'
```

The coordinate is a file and a JSON pointer, not a symbol. The probe recipe
lands in `.anchor/probes.toml` where you can read it; the anchor watches one
axis — `value`. When the supplier swaps peanut oil for sunflower oil,
`gmr check` exits 1 and hands the warning's rationale back **to a person**:
the ingredients moved, so whether the warning still applies must be re-decided,
not silently kept. The person re-reads (whole peanuts are still in the dish),
keeps the warning, and `gmr accept --why` seals that judgment.

**The inference loop.** An agent answers a customer and records what the
answer rested on:

```sh
gmr said 'told the customer the dish still contains whole peanuts…' \
  --on kung-pao-ingredients --saw <fact_address> \
  --depends 'all(anchors, not state.v.value)'
```

When the kitchen removes peanuts entirely, `gmr standing` exits 1: the
conclusion is no longer supported. Nobody re-reads an inference — it simply
stopped being true, and the next answer is composed fresh.

## Swap the prefix, keep everything else

`file://` is the offline stand-in. The same command shapes work against the
other fetched-fact families, and nothing downstream changes — same recipe
file, same `value` axis, same two loops:

```sh
gmr anchor 'https://api.example.com/menu#$.items.2.ingredients' --as kung-pao-live
gmr anchor 'sql://inventory.db#SELECT ingredients FROM dishes WHERE id=3' --as kung-pao-stock
```

And the memory does not have to be a file in this directory: `--record
mem0:<id>` (or any registered provider address) binds a record that lives in
your own memory system, with nothing copied here.

## The same loops from Python

```python
import gmr

g = gmr.open({"root": ".", "providers": {"git": True},
              "recipes": {"file": {"menu": {"path": "menu.json",
                                            "select": "$.items.2.ingredients"}}}})
reading = g.sample("kung-pao-ingredients")
g.bind("said:t1", ["kung-pao-ingredients"], "self_attested",
       {"saw": [reading["fact_address"]],
        "depends": "all(anchors, not state.v.value)"})
print(g.ground(["said:t1"])[0]["depends"])   # "holds" — until the kitchen moves
```
