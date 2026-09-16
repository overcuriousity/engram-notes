<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { forceLink, forceManyBody, forceSimulation, forceX, forceY, type ForceLink, type ForceManyBody, type ForceX, type ForceY, type Simulation, type SimulationNodeDatum } from "d3-force";
  import SlidersHorizontal from "@lucide/svelte/icons/sliders-horizontal";
  import X from "@lucide/svelte/icons/x";
  import { app } from "../lib/state.svelte";
  import { createNote, errorMessage, getGraphConfig, graph as loadGraph, search, semanticEdges, setGraphConfig, type Graph, type SemanticEdge } from "../lib/api";
  import { GROUP_COLORS, defaultSettings, easeStep, filterGraph, fitView, forces, groupColors, hexColor, hitRadius, labelAlpha, radius, readSettings, rgbInt, searchWords, type GraphSettings, type ViewGraph, type ViewNode } from "../lib/graph";
  import { resolvedTheme } from "../lib/snippets";
  import { weightBucket, type DrawEdge } from "../lib/semantic";

  let { local }: { local: boolean } = $props();

  interface Node extends SimulationNodeDatum { data: ViewNode; r: number }
  interface Link { source: Node; target: Node; kind: DrawEdge["kind"]; weight: number }
  type NumKey = { [K in keyof GraphSettings]: GraphSettings[K] extends number ? K : never }[keyof GraphSettings];

  let canvas = $state<HTMLCanvasElement>();
  let data = $state.raw<Graph>({ nodes: [], edges: [] });
  let settings = $state<GraphSettings>(defaultSettings());
  let content = $state.raw<Set<string> | null>(null);
  let semantic = $state.raw<SemanticEdge[]>([]);
  let panel = $state(false);
  let loaded = $state(false);
  // graph.json as read, so keys this app does not know are written back unchanged.
  let raw: Record<string, unknown> = {};

  // Simulation state lives outside Svelte's reactivity; d3 mutates it every tick.
  let nodes: Node[] = [];
  let links: Link[] = [];
  let byId = new Map<string, Node>();
  let degree = new Map<Node, number>();
  let sim: Simulation<Node, undefined> | undefined;
  let view = { x: 0, y: 0, k: 1 };
  let size = { w: 0, h: 0 };
  let hover: Node | null = null;
  let near = new Set<Node>();
  let press: { sx: number; sy: number; vx: number; vy: number; node: Node | null; moved: boolean } | null = null;
  let frame = 0;
  // A wheel notch or a fit sets a target the view slides to; a drag moves it outright.
  let target: { x: number; y: number; k: number } | null = null;
  let easing = 0;
  // While the first layout settles the view keeps it framed; a gesture takes over.
  let fitPending = false;
  // Theme colours, read from CSS once per theme change rather than once per frame.
  // The read happens in the paint that follows, so it cannot land before the
  // theme the app is switching to is on the document.
  let palette: Record<string, string> = {};
  let paletteStale = true;
  let prefersDark = $state(matchMedia("(prefers-color-scheme: dark)").matches);

  const say = (e: unknown) => app.say(errorMessage(e));
  const center = $derived(local ? app.lastNote : null);
  const semanticOn = $derived(local ? settings.showSemanticLocal : settings.showSemantic);
  const shown = $derived<ViewGraph>(local && !center ? { nodes: [], edges: [] } : filterGraph(data, settings, content, center, semantic));
  // Colour is painted, not laid out: editing a group must not rebuild the simulation.
  const colors = $derived(groupColors(data.nodes, settings.colorGroups));

  onMount(() => {
    const c = canvas!;
    getGraphConfig()
      .then((r) => {
        raw = r;
        settings = readSettings(r);
        loaded = true;
      })
      .catch(say);
    const observer = new ResizeObserver(resize);
    observer.observe(c);
    // Registered by hand: the listener must not be passive to stop the page scrolling.
    c.addEventListener("wheel", wheel, { passive: false });
    return () => {
      observer.disconnect();
      c.removeEventListener("wheel", wheel);
      sim?.stop();
      cancelAnimationFrame(frame);
      cancelAnimationFrame(easing);
    };
  });

  // The index changed.
  $effect(() => {
    void app.files;
    loadGraph().then((g) => (data = g)).catch(say);
  });

  // The local graph asks for the centre's edges; the global graph for the vault's.
  $effect(() => {
    void app.files;
    if (!semanticOn || (local && !center)) {
      semantic = [];
      return;
    }
    semanticEdges(local && center ? [center] : null)
      .then((e) => (semantic = e))
      .catch(say);
  });

  // Plain words also match note text, through full-text search.
  $effect(() => {
    const words = searchWords(settings.search).trim();
    if (!words) {
      content = null;
      return;
    }
    const t = setTimeout(() => search(words, 100000).then((r) => (content = new Set(r.hits.map((h) => h.path)))).catch(say), 200);
    return () => clearTimeout(t);
  });

  $effect(() => {
    const snap = $state.snapshot(settings);
    if (!loaded) return;
    const t = setTimeout(() => setGraphConfig({ ...raw, ...snap }).catch(say), 500);
    return () => clearTimeout(t);
  });

  // Only the drawn set rebuilds the simulation; the sliders below adjust it in place,
  // so dragging one no longer throws every node back into a fresh layout.
  $effect(() => {
    const g = shown;
    untrack(() => rebuild(g));
  });

  $effect(() => {
    const f = forces(settings);
    untrack(() => retune(f));
  });

  $effect(() => {
    void settings.nodeSizeMultiplier;
    untrack(() => {
      for (const n of nodes) n.r = radius(n.data, settings);
      draw();
    });
  });

  $effect(() => {
    void settings.lineSizeMultiplier;
    void settings.textFadeMultiplier;
    void settings.showArrow;
    draw();
  });

  $effect(() => {
    const mq = matchMedia("(prefers-color-scheme: dark)");
    const onChange = (e: MediaQueryListEvent) => (prefersDark = e.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  });

  // The theme as it reaches the screen: under "system", the default, the config
  // never changes and only the OS preference says the colours moved.
  $effect(() => {
    void resolvedTheme(app.config?.theme ?? "system", prefersDark);
    paletteStale = true;
    draw();
  });

  $effect(() => {
    void colors;
    draw();
  });

  function readPalette() {
    const css = getComputedStyle(canvas ?? document.documentElement);
    palette = {};
    for (const name of ["--accent", "--graph-line", "--graph-note", "--graph-tag", "--graph-attachment", "--fg", "--font-ui"]) palette[name] = css.getPropertyValue(name).trim();
  }

  // Nodes keep their positions across filter changes, so the layout does not jump.
  function rebuild(g: ViewGraph) {
    const old = byId;
    byId = new Map();
    nodes = g.nodes.map((d) => {
      const n: Node = old.get(d.id) ?? { data: d, r: 0 };
      n.data = d;
      n.r = radius(d, settings);
      byId.set(d.id, n);
      return n;
    });
    links = g.edges.map((e) => ({ source: byId.get(e.source)!, target: byId.get(e.target)!, kind: e.kind, weight: e.weight }));
    degree = new Map();
    for (const l of links) for (const n of [l.source, l.target]) degree.set(n, (degree.get(n) ?? 0) + 1);
    if (old.size === 0) fitPending = true;
    const f = forces(settings);
    sim?.stop();
    // Settles in a couple of seconds and damps harder than d3's default, so the
    // nodes glide into place instead of swinging past it.
    sim = forceSimulation(nodes)
      .alphaDecay(0.04)
      .velocityDecay(0.5)
      .force("link", forceLink<Node, Link>(links).distance(f.distance).strength(linkStrength(f.link)))
      .force("charge", forceManyBody<Node>().strength(f.charge).distanceMax(2000))
      .force("x", forceX<Node>(0).strength(f.center))
      .force("y", forceY<Node>(0).strength(f.center))
      .alpha(old.size ? 0.3 : 1)
      .on("tick", tick)
      .on("end", () => (fitPending = false));
    hover = null;
    near = new Set();
    draw();
  }

  // d3's default spreads a link's pull over both ends' degrees; a hub then pins its leaves.
  const linkStrength = (k: number) => (l: Link) => k / Math.min(degree.get(l.source)!, degree.get(l.target)!);

  function retune(f: ReturnType<typeof forces>) {
    if (!sim) return;
    (sim.force("link") as ForceLink<Node, Link>).distance(f.distance).strength(linkStrength(f.link));
    (sim.force("charge") as ForceManyBody<Node>).strength(f.charge);
    (sim.force("x") as ForceX<Node>).strength(f.center);
    (sim.force("y") as ForceY<Node>).strength(f.center);
    sim.alpha(Math.max(sim.alpha(), 0.3)).restart();
  }

  function tick() {
    if (fitPending) fit();
    draw();
  }

  function resize() {
    const c = canvas!;
    const box = c.getBoundingClientRect();
    if (!size.w) view = { ...view, x: box.width / 2, y: box.height / 2 };
    size = { w: box.width, h: box.height };
    const dpr = devicePixelRatio || 1;
    c.width = Math.round(box.width * dpr);
    c.height = Math.round(box.height * dpr);
    paletteStale = true;
    draw();
  }

  // Obsidian opens a graph showing all of it; ours keeps all of it in view as it settles.
  function fit() {
    if (!nodes.length || !size.w) return;
    target = fitView(nodes.map((n) => ({ x: n.x!, y: n.y!, r: n.r })), size.w, size.h);
    ease();
  }

  // One loop, started once and left to run: a gesture retargets faster than a frame
  // arrives, and cancelling per event starved the animation in the real window.
  function ease() {
    if (easing) return;
    const step = () => {
      const t = target;
      if (!t) {
        easing = 0;
        return;
      }
      const next = easeStep(view, t);
      view = next.view;
      if (next.done) {
        target = null;
        easing = 0;
      } else {
        easing = requestAnimationFrame(step);
      }
      draw();
    };
    easing = requestAnimationFrame(step);
  }

  function draw() {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(paint);
  }

  function paint() {
    const c = canvas;
    if (!c) return;
    const ctx = c.getContext("2d")!;
    if (paletteStale) {
      readPalette();
      paletteStale = false;
    }
    const color = (name: string) => palette[name] ?? "";
    const dpr = devicePixelRatio || 1;
    const { x: ox, y: oy, k } = view;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, c.width, c.height);
    ctx.setTransform(dpr * k, 0, 0, dpr * k, dpr * ox, dpr * oy);
    const dim = hover ? 0.25 : 1;

    ctx.lineWidth = Math.max(settings.lineSizeMultiplier, 0.5 / k);
    for (const lit of [false, true]) {
      ctx.beginPath();
      for (const l of links) {
        if (l.kind !== "link") continue;
        if ((l.source === hover || l.target === hover) !== lit) continue;
        ctx.moveTo(l.source.x!, l.source.y!);
        ctx.lineTo(l.target.x!, l.target.y!);
      }
      ctx.strokeStyle = color(lit ? "--accent" : "--graph-line");
      ctx.globalAlpha = lit ? 1 : dim;
      ctx.stroke();
    }

    // Learned and near: dashed, thicker where the tie is stronger. Batched by
    // width so a whole vault of them still costs a handful of paths.
    const dashed = links.filter((l) => l.kind !== "link");
    if (dashed.length) {
      ctx.setLineDash([4 / k, 3 / k]);
      for (const lit of [false, true]) {
        for (const bucket of [0, 1, 2]) {
          let any = false;
          ctx.beginPath();
          for (const l of dashed) {
            if ((l.source === hover || l.target === hover) !== lit) continue;
            if (weightBucket(l) !== bucket) continue;
            any = true;
            ctx.moveTo(l.source.x!, l.source.y!);
            ctx.lineTo(l.target.x!, l.target.y!);
          }
          if (!any) continue;
          ctx.lineWidth = Math.max((0.6 + bucket * 0.6) * settings.lineSizeMultiplier, 0.5 / k);
          ctx.strokeStyle = color(lit ? "--accent" : "--graph-line");
          ctx.globalAlpha = lit ? 1 : dim;
          ctx.stroke();
        }
      }
      ctx.setLineDash([]);
      ctx.lineWidth = Math.max(settings.lineSizeMultiplier, 0.5 / k);
    }

    if (settings.showArrow) {
      ctx.beginPath();
      for (const { source: a, target: b, kind } of links) {
        if (kind !== "link") continue;
        const dx = b.x! - a.x!;
        const dy = b.y! - a.y!;
        const len = Math.hypot(dx, dy) || 1;
        const [ux, uy] = [dx / len, dy / len];
        const tip = [b.x! - ux * b.r, b.y! - uy * b.r];
        const s = 3 + ctx.lineWidth * 2;
        ctx.moveTo(tip[0], tip[1]);
        ctx.lineTo(tip[0] - ux * s - uy * s * 0.6, tip[1] - uy * s + ux * s * 0.6);
        ctx.lineTo(tip[0] - ux * s + uy * s * 0.6, tip[1] - uy * s - ux * s * 0.6);
        ctx.closePath();
      }
      ctx.fillStyle = color("--graph-line");
      ctx.globalAlpha = dim;
      ctx.fill();
    }

    // One path per colour and opacity keeps ten thousand nodes cheap. A group's
    // colour is its own key; a hollow node stays hollow whatever its group.
    const groups = new Map<string, Node[]>();
    for (const n of nodes) {
      const accent = n === hover || near.has(n);
      const paint = accent ? "accent" : n.data.kind === "unresolved" ? "unresolved" : (colors.get(n.data.id) ?? n.data.kind);
      const key = `${paint}|${accent || !hover ? 1 : dim}`;
      const g = groups.get(key);
      if (g) g.push(n);
      else groups.set(key, [n]);
    }
    for (const [key, group] of groups) {
      const [kind, alpha] = key.split("|");
      ctx.globalAlpha = Number(alpha);
      ctx.beginPath();
      for (const n of group) {
        ctx.moveTo(n.x! + n.r, n.y!);
        ctx.arc(n.x!, n.y!, n.r, 0, 2 * Math.PI);
      }
      if (kind === "unresolved") {
        ctx.strokeStyle = color("--graph-note");
        ctx.lineWidth = 1.2;
        ctx.stroke();
      } else {
        ctx.fillStyle = kind.startsWith("#") ? kind : color(kind === "accent" ? "--accent" : `--graph-${kind}`);
        ctx.fill();
      }
    }

    const fade = labelAlpha(k, settings.textFadeMultiplier);
    ctx.font = `${12 / k}px ${color("--font-ui")}`;
    ctx.textAlign = "center";
    ctx.textBaseline = "top";
    ctx.fillStyle = color("--fg");
    for (const n of nodes) {
      const lit = n === hover || near.has(n);
      const a = lit ? 1 : hover ? fade * dim : fade;
      const sx = n.x! * k + ox;
      const sy = n.y! * k + oy;
      if (a < 0.02 || sx < -100 || sx > size.w + 100 || sy < -20 || sy > size.h + 20) continue;
      ctx.globalAlpha = a;
      ctx.fillText(n.data.title, n.x!, n.y! + n.r + 2 / k);
    }
    ctx.globalAlpha = 1;
  }

  function point(e: MouseEvent) {
    const b = canvas!.getBoundingClientRect();
    const sx = e.clientX - b.left;
    const sy = e.clientY - b.top;
    return { sx, sy, x: (sx - view.x) / view.k, y: (sy - view.y) / view.k };
  }

  function nodeAt(x: number, y: number): Node | null {
    let best: Node | null = null;
    let dist = Infinity;
    for (const n of nodes) {
      const d = Math.hypot(n.x! - x, n.y! - y);
      if (d < hitRadius(n.r, view.k) && d < dist) {
        best = n;
        dist = d;
      }
    }
    return best;
  }

  function setHover(n: Node | null) {
    if (n === hover) return;
    hover = n;
    near = new Set();
    for (const l of n ? links : []) {
      if (l.source === n) near.add(l.target);
      if (l.target === n) near.add(l.source);
    }
    canvas!.style.cursor = n ? "pointer" : "grab";
    draw();
  }

  function down(e: PointerEvent) {
    fitPending = false;
    cancelAnimationFrame(easing);
    easing = 0;
    target = null;
    const p = point(e);
    canvas!.setPointerCapture(e.pointerId);
    const node = nodeAt(p.x, p.y);
    press = { sx: p.sx, sy: p.sy, vx: view.x, vy: view.y, node, moved: false };
    if (node) {
      node.fx = node.x;
      node.fy = node.y;
      sim?.alphaTarget(0.3).restart();
    }
  }

  function move(e: PointerEvent) {
    const p = point(e);
    if (!press) return setHover(nodeAt(p.x, p.y));
    if (Math.hypot(p.sx - press.sx, p.sy - press.sy) > 3) press.moved = true;
    if (press.node) {
      press.node.fx = p.x;
      press.node.fy = p.y;
    } else {
      view = { ...view, x: press.vx + p.sx - press.sx, y: press.vy + p.sy - press.sy };
      draw();
    }
  }

  function up() {
    const p = press;
    press = null;
    if (!p?.node) return;
    p.node.fx = p.node.fy = null;
    sim?.alphaTarget(0);
    if (!p.moved) void openNode(p.node.data);
  }

  function wheel(e: WheelEvent) {
    e.preventDefault();
    fitPending = false;
    const p = point(e);
    const from = target ?? view;
    const k = Math.min(8, Math.max(0.05, from.k * Math.exp(-e.deltaY * 0.0015)));
    // Zoom about the pointer: the world point under it stays under it.
    const wx = (p.sx - from.x) / from.k;
    const wy = (p.sy - from.y) / from.k;
    target = { x: p.sx - wx * k, y: p.sy - wy * k, k };
    ease();
  }

  // A note opens beside the graph rather than over it, so the graph stays where it is;
  // Obsidian replaces the graph instead, and the user asked for the split.
  async function openNode(n: ViewNode) {
    try {
      if (n.kind === "tag") {
        settings.search = `tag:${n.id}`;
      } else if (n.kind === "unresolved") {
        const path = /\.md$/i.test(n.id) ? n.id : `${n.id}.md`;
        await createNote(path);
        await app.refresh();
        await app.openInOtherPane(path);
      } else {
        await app.openInOtherPane(n.id);
      }
    } catch (e) {
      say(e);
    }
  }
</script>

{#snippet slider(label: string, key: NumKey, min: number, max: number, step: number)}
  <label class="slider">{label}<input type="range" {min} {max} {step} bind:value={settings[key]} /></label>
{/snippet}

<div class="graph">
  <canvas
    bind:this={canvas}
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointerleave={() => {
      if (!press) setHover(null);
    }}
  ></canvas>
  {#if local && !center}<div class="graph-empty">Open a note to see its local graph</div>{/if}
  <div class="graph-controls" class:open={panel}>
    <button class="icon graph-toggle" title={panel ? "Close graph settings" : "Graph settings"} onclick={() => (panel = !panel)}>
      {#if panel}<X size={16} strokeWidth={1.75} />{:else}<SlidersHorizontal size={16} strokeWidth={1.75} />{/if}
    </button>
    {#if panel}
      <h4>Filters</h4>
      <input placeholder="Search files…" bind:value={settings.search} />
      {#if local}<label class="slider">Depth {settings.localDepth}<input type="range" min="1" max="3" step="1" bind:value={settings.localDepth} /></label>{/if}
      <label><input type="checkbox" bind:checked={settings.showTags} /> Tags</label>
      <label><input type="checkbox" bind:checked={settings.showAttachments} /> Attachments</label>
      <label><input type="checkbox" bind:checked={settings.hideUnresolved} /> Existing files only</label>
      <label><input type="checkbox" bind:checked={settings.showOrphans} /> Orphans</label>
      <h4>Groups</h4>
      {#each settings.colorGroups as g, i}
        <div class="group">
          <input placeholder="tag:#project" bind:value={g.query} />
          <input type="color" title="Colour" value={hexColor(g.color.rgb)} oninput={(e) => (g.color = { a: 1, rgb: rgbInt(e.currentTarget.value) })} />
          <button class="icon" title="Remove group" onclick={() => settings.colorGroups.splice(i, 1)}><X size={14} strokeWidth={1.75} /></button>
        </div>
      {/each}
      <button class="pick" onclick={() => settings.colorGroups.push({ query: "", color: { a: 1, rgb: GROUP_COLORS[settings.colorGroups.length % GROUP_COLORS.length] } })}>New group</button>
      <h4>Display</h4>
      <div class="pane-note">A node opens beside the graph, in the other split.</div>
      <label><input type="checkbox" bind:checked={settings.showArrow} /> Arrows</label>
      <label>
        <input
          type="checkbox"
          checked={semanticOn}
          onchange={(e) => {
            const on = e.currentTarget.checked;
            if (local) settings.showSemanticLocal = on;
            else settings.showSemantic = on;
          }} /> Semantic edges
      </label>
      {@render slider("Text fade threshold", "textFadeMultiplier", -3, 3, 0.1)}
      {@render slider("Node size", "nodeSizeMultiplier", 0.1, 5, 0.1)}
      {@render slider("Link thickness", "lineSizeMultiplier", 0.1, 5, 0.1)}
      <h4>Forces</h4>
      {@render slider("Center force", "centerStrength", 0, 1, 0.01)}
      {@render slider("Repel force", "repelStrength", 0, 20, 0.1)}
      {@render slider("Link force", "linkStrength", 0, 1, 0.01)}
      {@render slider("Link distance", "linkDistance", 30, 500, 1)}
    {/if}
  </div>
</div>
