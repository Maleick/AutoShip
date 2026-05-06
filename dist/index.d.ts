import type { PluginServer } from "./types.js";
export declare const id = "autoship";
export declare const repoRoot: string;
export declare const version: string;
export declare function server(): Promise<PluginServer>;
export * from "./types.js";
