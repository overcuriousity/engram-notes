<script lang="ts">
  import { app } from "../lib/state.svelte";
  import type { Node } from "../lib/layout";
  import Pane from "./Pane.svelte";
  import Workspace from "./Workspace.svelte";

  let { node }: { node: Node } = $props();

  // A divider trades size between the two children beside it.
  function drag(e: PointerEvent, i: number) {
    if (node.kind !== "split") return;
    e.preventDefault();
    const { id, dir } = node;
    const sizes = [...node.sizes];
    const box = (e.currentTarget as HTMLElement).parentElement!.getBoundingClientRect();
    const total = dir === "row" ? box.width : box.height;
    const start = dir === "row" ? e.clientX : e.clientY;
    const [a, b] = [sizes[i], sizes[i + 1]];
    const move = (m: PointerEvent) => {
      const d = ((dir === "row" ? m.clientX : m.clientY) - start) / total;
      const shift = Math.max(0.1 - a, Math.min(b - 0.1, d));
      const next = [...sizes];
      next[i] = a + shift;
      next[i + 1] = b - shift;
      app.resize(id, next);
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      app.persist();
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }
</script>

{#if node.kind === "pane"}
  <Pane pane={node} />
{:else}
  <div class="split {node.dir}">
    {#each node.children as child, i (`${child.kind}${child.id}`)}
      {#if i > 0}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="divider" onpointerdown={(e) => drag(e, i - 1)}></div>
      {/if}
      <div class="cell" style="flex:{node.sizes[i]} 1 0px"><Workspace node={child} /></div>
    {/each}
  </div>
{/if}
