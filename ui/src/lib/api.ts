import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface FileEntry { path: string; mtime_ms: number; size: number; is_markdown: boolean }
export interface RebuildStats { added: number; updated: number; removed: number; unchanged: number }
export interface AppConfig {
  editor: { default_mode: "live" | "source" | "reading"; indent: number };
  daily_notes: { folder: string; template: string | null; format: string };
  hotkeys: Record<string, string>;
  theme: "system" | "light" | "dark";
  search: { candidate_multiplier: number; rrf_k: number; cliff_factor: number; cliff_min_share: number; similarity_floor: number };
  memory: {
    enabled: boolean; activation_half_life_days: number; assoc_half_life_days: number;
    sitting_gap_secs: number; assoc_window_secs: number; assoc_show: number;
    prime_margin: number; prime_lift: number; spread_max: number;
  };
  embed: { model_dir: string | null; batch: number };
}
export interface VaultInfo { root: string; config: AppConfig; stats: RebuildStats; index_recreated: boolean; watch_error: string | null }
export interface NoteText { path: string; text: string; mtime_ms: number }
export interface LinkRow {
  src_path: string; target_raw: string; target_path: string | null; kind: string;
  heading: string | null; block: string | null; alias: string | null; line: number; context: string;
}
export interface Unresolved { target: string; count: number }
export interface TagCount { tag: string; count: number }
export interface PropertyCount { key: string; count: number }
export interface Hit {
  path: string; title: string; snippet: string; heading: string | null; line: number;
  similarity: number | null; score: number; past_divider: boolean; primed: boolean;
}
export interface Associated { path: string; title: string; via: string; cue: string | null; strength: number }
export interface SearchResults { hits: Hit[]; associated: Associated[] }
export interface SimilarNote { path: string; title: string; heading: string; text: string; similarity: number }
export interface Related { associated: Associated[]; similar: SimilarNote[]; suggested: SimilarNote[] }
export interface SemanticEdge { source: string; target: string; weight: number; kind: "assoc" | "similar" }
export interface EmbedStatus { model: string | null; state: "off" | "loading" | "ready" | "error"; pending: number; error: string | null }
export type EventKind = "open" | "open_from_search" | "follow_link" | "search"
export interface RenamePlan { from: string; to: string; affected: string[] }
export interface Change { path: string; kind: "changed" | "removed" }
export interface CommandError { code: string; message: string }
export interface GraphNode { id: string; title: string; kind: "note" | "attachment" | "unresolved"; tags: string[] }
export interface GraphEdge { source: string; target: string }
export interface Graph { nodes: GraphNode[]; edges: GraphEdge[] }
export interface SortKey { property: string; direction: "ASC" | "DESC" }
export interface Column { id: string; label: string; editable: boolean }
export interface BaseRow { path: string; cells: unknown[] }
export interface BaseTable { views: string[]; view: number; columns: Column[]; rows: BaseRow[]; sort: SortKey[]; errors: string[] }

export const recentVaults = () => invoke<string[]>("recent_vaults");
export const startupVault = () => invoke<string | null>("startup_vault");
export const openVault = (path: string) => invoke<VaultInfo>("open_vault", { path });
export const listFiles = () => invoke<FileEntry[]>("list_files");
export const readNote = (path: string) => invoke<NoteText>("read_note", { path });
export const writeNote = (path: string, text: string) => invoke<number>("write_note", { path, text });
export const createNote = (path: string, text = "") => invoke<void>("create_note", { path, text });
export const createFolder = (path: string) => invoke<void>("create_folder", { path });
export const deleteFile = (path: string) => invoke<void>("delete_file", { path });
export const planRename = (from: string, to: string) => invoke<RenamePlan>("plan_rename", { from, to });
export const applyRename = (plan: RenamePlan) => invoke<void>("apply_rename", { plan });
export const backlinks = (path: string) => invoke<LinkRow[]>("backlinks", { path });
export const outgoing = (path: string) => invoke<LinkRow[]>("outgoing", { path });
export const unresolved = () => invoke<Unresolved[]>("unresolved");
export const resolveLink = (target: string) => invoke<string | null>("resolve_link", { target });
export const titles = () => invoke<[string, string][]>("titles");
export const tags = () => invoke<TagCount[]>("tags");
export const allProperties = () => invoke<PropertyCount[]>("all_properties");
export const properties = (path: string) => invoke<Record<string, unknown>>("properties", { path });
export const setProperty = (path: string, key: string, value: unknown) => invoke<void>("set_property", { path, key, value });
export const search = (query: string, limit = 50) => invoke<SearchResults>("search", { query, limit });
export const recordEvent = (kind: EventKind, path?: string, query?: string) =>
  invoke<void>("record_event", { kind, path: path ?? null, query: query ?? null });
export const related = (path: string) => invoke<Related>("related", { path });
export const forgetMemory = () => invoke<void>("forget_memory");
export const semanticEdges = (paths: string[] | null, topK = 3) =>
  invoke<SemanticEdge[]>("semantic_edges", { paths, topK });
export const embedStatus = () => invoke<EmbedStatus>("embed_status");
export const typing = () => invoke<void>("typing");
export const setModelDir = (dir: string | null) => invoke<void>("set_model_dir", { dir });
export const getConfig = () => invoke<AppConfig>("get_config");
export const setConfig = (config: AppConfig) => invoke<void>("set_config", { config });
export const getWorkspace = () => invoke<Record<string, unknown>>("get_workspace");
export const setWorkspace = (workspace: Record<string, unknown>) => invoke<void>("set_workspace", { workspace });
export const dailyNote = () => invoke<string>("daily_note");
export const rescan = () => invoke<RebuildStats>("rescan");
export const attachmentPath = (path: string) => invoke<string>("attachment_path", { path });
export const openExternal = (path: string) => invoke<void>("open_external", { path });
export const anchorLine = (path: string, fragment: string) => invoke<number | null>("anchor_line", { path, fragment });
export const listFolders = () => invoke<string[]>("list_folders");
export const graph = () => invoke<Graph>("graph");
export const getGraphConfig = () => invoke<Record<string, unknown>>("get_graph_config");
export const setGraphConfig = (config: Record<string, unknown>) => invoke<void>("set_graph_config", { config });
export const runBase = (path: string, view: number) => invoke<BaseTable>("run_base", { path, view });
export const setBaseSort = (path: string, view: number, sort: SortKey[]) => invoke<void>("set_base_sort", { path, view, sort });

export const onIndexChanged = (f: (c: Change[]) => void): Promise<UnlistenFn> =>
  listen<Change[]>("index-changed", (e) => f(e.payload));
export const onFileChanged = (f: (c: Change) => void): Promise<UnlistenFn> =>
  listen<Change>("file-changed", (e) => f(e.payload));
export const onWatchFailed = (f: (message: string) => void): Promise<UnlistenFn> =>
  listen<string>("watch-failed", (e) => f(e.payload));
export const onEmbedStatus = (f: (s: EmbedStatus) => void): Promise<UnlistenFn> =>
  listen<EmbedStatus>("embed-status", (e) => f(e.payload));

export function errorMessage(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return String((e as CommandError).message);
  return String(e);
}
