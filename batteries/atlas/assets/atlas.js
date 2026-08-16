(function () {
  "use strict";

  var DATA = JSON.parse(document.getElementById("atlas-data").textContent);
  var TONE_ORDER = ["alarm", "notice", "calm", "muted"];
  var TONE_RANK = { alarm: 0, notice: 1, calm: 2, muted: 3 };

  var byId = Object.create(null);
  DATA.nodes.forEach(function (n) {
    byId[n.id] = n;
    n.neighbours = [];
  });
  DATA.edges.forEach(function (e) {
    byId[e.source].neighbours.push({ id: e.target, kind: e.kind });
    byId[e.target].neighbours.push({ id: e.source, kind: e.kind });
  });

  var anchors = DATA.nodes.filter(function (n) {
    return n.kind === "anchor";
  });
  var memories = DATA.nodes.filter(function (n) {
    return n.kind === "memory";
  });

  function css(name) {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  }

  function el(tag, cls, text) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined && text !== null) e.textContent = text;
    return e;
  }

  function neighboursOf(node, kind) {
    var seen = Object.create(null);
    var out = [];
    node.neighbours.forEach(function (link) {
      var other = byId[link.id];
      if (!other || seen[other.id]) return;
      if (kind && other.kind !== kind) return;
      seen[other.id] = true;
      out.push(other);
    });
    return out;
  }

  var state = {
    selected: null,
    groups: new Set(),
    tones: new Set(),
    query: ""
  };

  function matchesFilters(node) {
    if (state.groups.size && !(node.group && state.groups.has(node.group))) return false;
    if (state.tones.size && !state.tones.has(node.tone)) return false;
    if (state.query) {
      var hay = (node.id + " " + node.label + " " + (node.badge || "")).toLowerCase();
      if (hay.indexOf(state.query) === -1) return false;
    }
    return true;
  }

  function visibleAnchors() {
    return anchors.filter(matchesFilters);
  }

  var toneColor = {};
  function readToneColors() {
    toneColor = {
      alarm: css("--alarm"),
      notice: css("--notice"),
      calm: css("--calm"),
      muted: css("--muted")
    };
  }
  readToneColors();

  function buildStyle() {
    return [

      {
        selector: "node",
        style: {
          "background-color": function (n) {
            return toneColor[n.data("tone")] || toneColor.calm;
          },
          width: function (n) {
            return 13 + n.data("weight") * 2.6;
          },
          height: function (n) {
            return 13 + n.data("weight") * 2.6;
          },
          "border-width": 0,
          label: "data(label)",
          "font-size": 9,
          "font-family": css("--font-mono") || "monospace",
          color: css("--ink-dim"),
          "text-opacity": 0,
          "text-margin-y": -4,
          "text-valign": "top",
          "text-halign": "center",
          "text-max-width": 150,
          "text-wrap": "ellipsis",
          "overlay-opacity": 0
        }
      },
      { selector: 'node[kind="anchor"]', style: { shape: "round-diamond" } },
      { selector: 'node[kind="memory"]', style: { shape: "ellipse" } },
      {
        selector: "edge",
        style: {
          width: 1,
          "line-color": css("--edge"),
          "curve-style": "straight",
          opacity: 0.55
        }
      },
      {
        selector: 'edge[kind="reference"]',
        style: { "line-style": "dashed", opacity: 0.4 }
      },
      { selector: ".dim", style: { opacity: 0.07, "text-opacity": 0 } },
      {
        selector: "node.lit",
        style: {
          "border-width": 1.5,
          "border-color": css("--accent"),
          "text-opacity": 1,
          "z-index": 20
        }
      },
      {
        selector: "edge.lit",
        style: { "line-color": css("--accent"), width: 2, opacity: 1, "z-index": 20 }
      },
      {
        selector: "node.picked",
        style: {
          "border-width": 2,
          "border-color": css("--accent"),
          "overlay-color": css("--accent"),
          "overlay-opacity": 0.22,
          "overlay-padding": 9,
          "text-opacity": 1,
          "font-weight": "bold",
          "z-index": 30
        }
      }
    ];
  }

  var cy = cytoscape({
    container: document.getElementById("cy"),
    elements: {
      nodes: DATA.nodes.map(function (n) {
        return {
          data: {
            id: n.id,
            label: n.label,
            kind: n.kind,
            tone: n.tone,
            weight: Math.min(9, neighboursOf(n).length)
          }
        };
      }),
      edges: DATA.edges.map(function (e, i) {
        return {
          data: { id: "e" + i, source: e.source, target: e.target, kind: e.kind }
        };
      })
    },
    minZoom: 0.05,
    maxZoom: 4,
    style: buildStyle(),
    layout: { name: "preset" }
  });

  function runLayout() {
    cy.layout({
      name: "fcose",
      animate: false,
      quality: "default",
      randomize: true,
      nodeSeparation: 90,
      idealEdgeLength: 62,
      nodeRepulsion: 6500,
      gravity: 0.22,
      packComponents: true
    }).run();
    cy.fit(undefined, 40);
  }

  function applyFilters() {
    var pass = Object.create(null);
    DATA.nodes.forEach(function (n) {
      if (n.kind === "anchor") pass[n.id] = matchesFilters(n);
    });
    memories.forEach(function (m) {
      pass[m.id] = neighboursOf(m, "anchor").some(function (a) {
        return pass[a.id];
      });
      if (!m.neighbours.length) pass[m.id] = matchesFilters(m);
    });
    var anyFilter = state.groups.size || state.tones.size || state.query;
    cy.batch(function () {
      cy.nodes().forEach(function (n) {
        n.toggleClass("dim", Boolean(anyFilter) && !pass[n.id()]);
      });
      cy.edges().forEach(function (e) {
        e.toggleClass(
          "dim",
          Boolean(anyFilter) && (!pass[e.source().id()] || !pass[e.target().id()])
        );
      });
    });
    renderList();
    renderStats();
  }

  function highlight(node) {
    cy.batch(function () {
      cy.elements().removeClass("lit picked");
      if (!node) return;
      var self = cy.getElementById(node.id);
      self.addClass("picked");
      self.connectedEdges().addClass("lit").connectedNodes().addClass("lit");
    });
  }

  function select(id, focus) {
    state.selected = id && byId[id] ? id : null;
    highlight(state.selected ? byId[state.selected] : null);
    renderPanel();
    renderList();
    if (focus && state.selected) {
      var target = cy.getElementById(state.selected);
      cy.animate(
        { center: { eles: target }, zoom: Math.max(cy.zoom(), 1.1) },
        { duration: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : 260 }
      );
    }
  }

  var listEl = document.getElementById("list");

  function toneDot(node) {
    var d = el("span", "dot" + (node.kind === "anchor" ? " diamond" : ""));
    d.style.background = toneColor[node.tone] || toneColor.calm;
    return d;
  }

  function renderList() {
    var shown = visibleAnchors();
    listEl.textContent = "";
    if (!shown.length) {
      listEl.appendChild(el("div", "empty", "No anchor matches these filters."));
      return;
    }
    var grouped = new Map();
    shown.forEach(function (a) {
      var g = a.group || "ungrouped";
      if (!grouped.has(g)) grouped.set(g, []);
      grouped.get(g).push(a);
    });
    Array.from(grouped.keys())
      .sort()
      .forEach(function (g) {
        var rows = grouped.get(g).sort(function (x, y) {
          var t = TONE_RANK[x.tone] - TONE_RANK[y.tone];
          return t !== 0 ? t : x.label.localeCompare(y.label);
        });
        var head = el("div", "group-head");
        head.appendChild(el("span", null, g));
        head.appendChild(el("span", null, String(rows.length)));
        listEl.appendChild(head);
        rows.forEach(function (a) {
          var row = el("button", "row");
          row.type = "button";
          row.setAttribute("role", "option");
          row.setAttribute("aria-selected", String(state.selected === a.id));
          row.appendChild(toneDot(a));
          var text = el("div", "row-text");
          text.appendChild(el("div", "row-name", a.label));
          var sub = a.id.replace(/^anchor:/, "");
          if (sub !== a.label) {
            var subEl = el("div", "row-sub", sub);
            subEl.title = sub;
            text.appendChild(subEl);
          }
          row.title = sub;
          row.appendChild(text);
          var tail = el("div", "row-tail");
          if (a.badge) tail.appendChild(el("span", "badge t-" + a.tone, a.badge));
          var bound = neighboursOf(a, "memory").length;
          if (bound > 1) tail.appendChild(el("span", "count", String(bound)));
          row.appendChild(tail);
          row.addEventListener("click", function () {
            select(a.id, true);
          });
          listEl.appendChild(row);
        });
      });
  }

  var panelEl = document.getElementById("panel");

  function proseFrom(html) {
    var box = el("div", "prose");
    box.innerHTML = html;
    box.querySelectorAll("a[data-node]").forEach(function (link) {
      var target = link.getAttribute("data-node");
      if (!byId[target]) {
        link.classList.add("dead");
        link.title = "no memory by that name in this atlas";
        return;
      }
      link.addEventListener("click", function (ev) {
        ev.preventDefault();
        select(target, true);
      });
    });
    return box;
  }

  function memoryBlock(m, withSource) {
    var wrap = el("div", "memory");
    if (withSource) wrap.appendChild(el("div", "src", m.label));
    if (m.detail) {
      wrap.appendChild(proseFrom(m.detail));
    } else {
      wrap.appendChild(el("p", "placeholder", "This memory has no readable text."));
    }
    return wrap;
  }

  function renderPanel() {
    panelEl.textContent = "";
    panelEl.scrollTop = 0;

    if (!state.selected) {
      var intro = el("div", "placeholder");
      intro.appendChild(
        el(
          "p",
          null,
          "Pick an anchor on the left or a node in the graph. Diamonds are anchors, circles are memories; colour is how loudly it is asking to be looked at."
        )
      );
      var shared = anchors
        .filter(function (a) {
          return neighboursOf(a, "memory").length > 1;
        })
        .sort(function (x, y) {
          return neighboursOf(y, "memory").length - neighboursOf(x, "memory").length;
        });
      if (shared.length) {
        var sec = el("div", "section");
        sec.appendChild(el("h3", null, "Anchors several memories watch"));
        shared.forEach(function (a) {
          var b = el("button", "jump", a.label);
          b.type = "button";
          b.appendChild(el("span", "count", String(neighboursOf(a, "memory").length)));
          b.addEventListener("click", function () {
            select(a.id, true);
          });
          sec.appendChild(b);
        });
        intro.appendChild(sec);
      }
      panelEl.appendChild(intro);
      return;
    }

    var node = byId[state.selected];
    var tag = el("div", "kind-tag");
    tag.appendChild(toneDot(node));
    tag.appendChild(document.createTextNode(node.kind));
    panelEl.appendChild(tag);
    panelEl.appendChild(el("h2", null, node.label));
    if (node.id.replace(/^(anchor|memory):/, "") !== node.label) {
      panelEl.appendChild(el("p", "coord", node.id.replace(/^(anchor|memory):/, "")));
    }

    if (node.facts && node.facts.length) {
      var dl = el("dl", "facts");
      node.facts.forEach(function (f) {
        dl.appendChild(el("dt", null, f.label));
        dl.appendChild(el("dd", null, f.value));
      });
      panelEl.appendChild(dl);
    }

    if (node.kind === "anchor") {
      var bound = neighboursOf(node, "memory");
      var sec = el("div", "section");
      sec.appendChild(
        el("h3", null, bound.length === 1 ? "The memory on it" : bound.length + " memories on it")
      );
      if (!bound.length) {
        sec.appendChild(el("p", "placeholder", "Nothing is bound here — this anchor is barren."));
      }
      bound.forEach(function (m) {
        sec.appendChild(memoryBlock(m, bound.length > 1));
      });
      panelEl.appendChild(sec);
      return;
    }

    if (node.detail) {
      panelEl.appendChild(proseFrom(node.detail));
    }
    var about = neighboursOf(node, "anchor");
    if (about.length) {
      var sec2 = el("div", "section");
      sec2.appendChild(el("h3", null, "About " + about.length + (about.length === 1 ? " anchor" : " anchors")));
      about.forEach(function (a) {
        var b = el("button", "jump");
        b.type = "button";
        b.appendChild(toneDot(a));
        b.appendChild(document.createTextNode(a.label));
        b.addEventListener("click", function () {
          select(a.id, true);
        });
        sec2.appendChild(b);
      });
      panelEl.appendChild(sec2);
    }
    var refs = node.neighbours.filter(function (l) {
      return l.kind === "reference";
    });
    if (refs.length) {
      var sec3 = el("div", "section");
      sec3.appendChild(el("h3", null, "Referenced memories"));
      refs.forEach(function (l) {
        var other = byId[l.id];
        if (!other) return;
        var b = el("button", "jump", other.label);
        b.type = "button";
        b.addEventListener("click", function () {
          select(other.id, true);
        });
        sec3.appendChild(b);
      });
      panelEl.appendChild(sec3);
    }
  }

  function renderStats() {
    var host = document.getElementById("stats");
    host.textContent = "";
    var shownAnchors = visibleAnchors();
    var counts = { alarm: 0, notice: 0 };
    anchors.forEach(function (a) {
      if (counts[a.tone] !== undefined) counts[a.tone] += 1;
    });
    var shared = anchors.filter(function (a) {
      return neighboursOf(a, "memory").length > 1;
    }).length;
    var spanning = memories.filter(function (m) {
      return neighboursOf(m, "anchor").length > 1;
    }).length;

    [
      ["anchors", shownAnchors.length === anchors.length ? anchors.length : shownAnchors.length + "/" + anchors.length, ""],
      ["memories", memories.length, ""],
      ["bindings", DATA.edges.filter(function (e) { return e.kind === "binding"; }).length, ""],
      ["shared anchors", shared, ""],
      ["multi-anchor notes", spanning, ""],
      ["needs a look", counts.notice, "t-notice"],
      ["failing", counts.alarm, "t-alarm"]
    ].forEach(function (row) {
      var s = el("div", "stat" + (row[2] ? " " + row[2] : ""));
      s.appendChild(el("b", null, String(row[1])));
      s.appendChild(el("span", null, row[0]));
      host.appendChild(s);
    });
  }

  function renderLegend() {
    var host = document.getElementById("legend");
    host.textContent = "";
    var seen = new Map();
    DATA.nodes.forEach(function (n) {
      if (!n.badge) return;
      if (!seen.has(n.badge)) seen.set(n.badge, { tone: n.tone, n: 0 });
      seen.get(n.badge).n += 1;
    });
    var kinds = [
      ["anchor", "diamond"],
      ["memory", ""]
    ];
    kinds.forEach(function (k) {
      var row = el("div", "legend-row");
      var d = el("span", "dot " + k[1]);
      d.style.background = css("--ink-faint");
      row.appendChild(d);
      row.appendChild(document.createTextNode(k[0]));
      host.appendChild(row);
    });
    var e1 = el("div", "legend-row");
    e1.appendChild(el("span", "line"));
    e1.appendChild(document.createTextNode("binding"));
    host.appendChild(e1);
    var e2 = el("div", "legend-row");
    e2.appendChild(el("span", "line dashed"));
    e2.appendChild(document.createTextNode("reference"));
    host.appendChild(e2);

    if (!seen.size) return;
    host.appendChild(el("div", "legend-sep"));
    Array.from(seen.entries())
      .sort(function (a, b) {
        var t = TONE_RANK[a[1].tone] - TONE_RANK[b[1].tone];
        return t !== 0 ? t : a[0].localeCompare(b[0]);
      })
      .forEach(function (entry) {
        var row = el("div", "legend-row");
        var d = el("span", "dot");
        d.style.background = toneColor[entry[1].tone] || toneColor.calm;
        row.appendChild(d);
        row.appendChild(document.createTextNode(entry[0]));
        row.appendChild(el("span", "count", String(entry[1].n)));
        host.appendChild(row);
      });
  }

  function renderChips() {
    var host = document.getElementById("chips");
    host.textContent = "";
    var groups = new Map();
    anchors.forEach(function (a) {
      var g = a.group || "ungrouped";
      groups.set(g, (groups.get(g) || 0) + 1);
    });
    Array.from(groups.keys())
      .sort()
      .forEach(function (g) {
        var chip = el("button", "chip");
        chip.type = "button";
        chip.setAttribute("aria-pressed", String(state.groups.has(g)));
        chip.appendChild(document.createTextNode(g));
        chip.appendChild(el("span", "n", String(groups.get(g))));
        chip.addEventListener("click", function () {
          if (state.groups.has(g)) state.groups.delete(g);
          else state.groups.add(g);
          chip.setAttribute("aria-pressed", String(state.groups.has(g)));
          applyFilters();
        });
        host.appendChild(chip);
      });

    var toneHost = document.getElementById("tones");
    toneHost.textContent = "";
    var toneCounts = {};
    anchors.forEach(function (a) {
      toneCounts[a.tone] = (toneCounts[a.tone] || 0) + 1;
    });
    TONE_ORDER.forEach(function (t) {
      if (!toneCounts[t]) return;
      var chip = el("button", "chip");
      chip.type = "button";
      chip.setAttribute("aria-pressed", String(state.tones.has(t)));
      var d = el("span", "dot");
      d.style.background = toneColor[t];
      chip.appendChild(d);
      chip.appendChild(document.createTextNode(t));
      chip.appendChild(el("span", "n", String(toneCounts[t])));
      chip.addEventListener("click", function () {
        if (state.tones.has(t)) state.tones.delete(t);
        else state.tones.add(t);
        chip.setAttribute("aria-pressed", String(state.tones.has(t)));
        applyFilters();
      });
      toneHost.appendChild(chip);
    });
  }

  cy.on("tap", "node", function (ev) {
    select(ev.target.id(), false);
  });
  cy.on("tap", function (ev) {
    if (ev.target === cy) select(null, false);
  });

  document.getElementById("refit").addEventListener("click", function () {
    cy.fit(undefined, 40);
  });

  var search = document.getElementById("q");
  search.addEventListener("input", function () {
    state.query = search.value.trim().toLowerCase();
    applyFilters();
  });
  search.addEventListener("keydown", function (ev) {
    if (ev.key === "Escape") {
      search.value = "";
      state.query = "";
      applyFilters();
    }
  });

  function onThemeChange() {
    readToneColors();
    cy.style(buildStyle());
    renderLegend();
    renderChips();
    renderList();
  }
  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", onThemeChange);
  new MutationObserver(onThemeChange).observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-theme"]
  });

  runLayout();
  renderChips();
  renderLegend();
  renderStats();
  renderList();
  renderPanel();
})();
