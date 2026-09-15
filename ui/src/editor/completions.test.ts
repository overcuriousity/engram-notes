import { describe, expect, it, vi } from "vitest";
import { EditorState } from "@codemirror/state";
import { CompletionContext } from "@codemirror/autocomplete";
import { wikiCompletion } from "./completions";

function ctx(doc: string) {
  const state = EditorState.create({ doc });
  return new CompletionContext(state, doc.length, true);
}

describe("wikiCompletion", () => {
  it("lists candidates with meaning marked and inserts the stem", async () => {
    const candidates = vi.fn(async () => [
      { path: "a/VAT fraud chain.md", title: "VAT fraud chain", kind: "meaning" as const, primed: false },
      { path: "Carousel.md", title: "Carousel", kind: "text" as const, primed: true },
    ]);
    const src = wikiCompletion(candidates, () => {});
    const r = await src(ctx("see [[car"));
    expect(candidates).toHaveBeenCalledWith("car");
    expect(r?.from).toBe(4);
    expect(r?.options.map((o) => [o.label, o.apply, o.detail])).toEqual([
      ["VAT fraud chain", "[[VAT fraud chain]]", "meaning · a/VAT fraud chain.md"],
      ["Carousel", "[[Carousel]]", "Carousel.md"],
    ]);
  });

  it("hands [[^^ to the block search and offers nothing itself", async () => {
    const onBlock = vi.fn();
    const src = wikiCompletion(async () => [], onBlock);
    const r = await src(ctx("x [[^^shell"));
    expect(r).toBeNull();
    expect(onBlock).toHaveBeenCalledWith(2, 11, "shell");
  });

  it("is quiet outside a link", async () => {
    const src = wikiCompletion(async () => [], () => {});
    expect(await src(ctx("plain text"))).toBeNull();
  });
});
