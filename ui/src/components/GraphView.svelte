<script lang="ts">
  import { onMount } from "svelte";
  import { forceLink, forceManyBody, forceSimulation, forceX, forceY, type Simulation, type SimulationNodeDatum } from "d3-force";
  import { app } from "../lib/state.svelte";
  import { createNote, errorMessage, getGraphConfig, graph as loadGraph, search, semanticEdges, setGraphConfig, type Graph, type SemanticEdge } from "../lib/api";
  import { DEFAULTS, filterGraph, forces, labelAlpha, radius, readSettings, searchWords, type GraphSettings, type ViewGraph, type ViewNode } from "../lib/graph";
  import { weightBucket, type DrawEdge } from "../lib/semantic";

  let { local }: { local: boolean } = $props();

  interface Node extends SimulationNodeDatum { data: ViewNode; r: number }
  interface Link { source: Node; target: Node; kind: DrawEdge["kind"]; weight: number }
  type NumKey = { [K in keyof GraphSettings]: GraphSettings[K] extends number ? K : never }[keyof GraphSettings];

  let canvas = $state<HTMLCanvasElement>();
  let data = $state.raw<Graph>({ nodes: [], edges: [] });
  let settings = $state<GraphSettings>({ ...DEFAULTS });
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
  let sim: Simulation<Node, undefined> | undefined;
  let view = { x: 0, y: 0, k: 1 };
  let size = { w: 0, h: 0 };
  let hover: Node | null = null;
  let near = new Set<Node>();
  let press: { sx: number; sy: number; vx: number; vy: number; node: Node | null; moved: boolean } | null = null;
  let frame = 0;

  const say = (e: unknown) => app.say(errorMessage(e));
  const center = $derived(local ? app.lastNote : null);
  const semanticOn = $derived(local ? settings.showSemanticLocal : settings.showSemantic);
  const shown = $derived<ViewGraph>(local && !center ? { nodes: [], edges: [] } : filterGraph(data, settings, content, center, semantic));

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

  $effect(() => rebuild(shown, settings));

  $effect(() => {
    void app.config?.theme;
    draw();
  });

  // Nodes keep their positions across filter changes, so the layout does not jump.
  function rebuild(g: ViewGraph, s: GraphSettings) {
    const old = byId;
    byId = new Map();
    nodes = g.nodes.map((d) => {
      const n: Node = old.get(d.id) ?? { data: d, r: 0 };
      n.data = d;
      n.r = radius(d, s);
      byId.set(d.id, n);
      return n;
    });
    links = g.edges.map((e) => ({ source: byId.get(e.source)!, target: byId.get(e.target)!, kind: e.kind, weight: e.weight }));
    const degree = new Map<Node, number>();
    for (const l of links) for (const n of [l.source, l.target]) degree.set(n, (degree.get(n) ?? 0) + 1);
    if (old.size === 0) view.k = Math.min(1.5, Math.max(0.15, Math.sqrt(30 / Math.max(1, nodes.length))));
    const f = forces(s);
    sim?.stop();
    sim = forceSimulation(nodes)
      .force("link", forceLink<Node, Link>(links).distance(f.distance).strength((l) => f.link / Math.min(degree.get(l.source)!, degree.get(l.target)!)))
      .force("charge", forceManyBody<Node>().strength(f.charge).distanceMax(2000))
      .force("x", forceX<Node>(0).strength(f.center))
      .force("y", forceY<Node>(0).strength(f.center))
      .alpha(old.size ? 0.3 : 1)
      .on("tick", draw);
    hover = null;
    near = new Set();
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
    draw();
  }

  function draw() {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(paint);
  }

  function paint() {
    const c = canvas;
    if (!c) return;
    const ctx = c.getContext("2d")!;
    const css = getComputedStyle(c);
    const color = (name: string) => css.getPropertyValue(name).trim();
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

    // One path per colour and opacity keeps ten thousand nodes cheap.
    const groups = new Map<string, Node[]>();
    for (const n of nodes) {
      const accent = n === hover || near.has(n);
      const key = `${accent ? "accent" : n.data.kind}|${accent || !hover ? 1 : dim}`;
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
        ctx.fillStyle = color(kind === "accent" ? "--accent" : `--graph-${kind}`);
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
      if (d < n.r + 4 / view.k && d < dist) {
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
    const p = point(e);
    const k = Math.min(8, Math.max(0.05, view.k * Math.exp(-e.deltaY * 0.0015)));
    view = { x: p.sx - p.x * k, y: p.sy - p.y * k, k };
    draw();
  }

  // As in Obsidian: a note opens, an unresolved link creates its note, a tag searches for itself.
  async function openNode(n: ViewNode) {
    try {
      if (n.kind === "tag") {
        settings.search = `tag:${n.id}`;
      } else if (n.kind === "unresolved") {
        const path = /\.md$/i.test(n.id) ? n.id : `${n.id}.md`;
        await createNote(path);
        await app.refresh();
        await app.openNote(path);
      } else {
        await app.openNote(n.id);
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
    <button class="graph-toggle" title="Graph settings" onclick={() => (panel = !panel)}>
      {#if panel}×{:else}<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><circle cx="12" cy="12" r="3" /><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1 7 17M17 7l2.1-2.1" /></svg>{/if}
    </button>
    {#if panel}
      <h4>Filters</h4>
      <input placeholder="Search files…" bind:value={settings.search} />
      {#if local}<label class="slider">Depth {settings.localDepth}<input type="range" min="1" max="3" step="1" bind:value={settings.localDepth} /></label>{/if}
      <label><input type="checkbox" bind:checked={settings.showTags} /> Tags</label>
      <label><input type="checkbox" bind:checked={settings.showAttachments} /> Attachments</label>
      <label><input type="checkbox" bind:checked={settings.hideUnresolved} /> Existing files only</label>
      <label><input type="checkbox" bind:checked={settings.showOrphans} /> Orphans</label>
      <h4>Display</h4>
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
