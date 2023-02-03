import { writable } from "svelte/store";
import type { LayoutLoad } from "./$types";
import { building } from "$app/environment";
import type { Project } from "$lib/projects";

export const ssr = false;
export const prerender = true;
export const csr = true;

export const load: LayoutLoad = async () => ({
    // tauri apis require window reference which doesn't exist during ssr, so dynamic import here.
    projects: building
        ? writable<Project[]>([])
        : (await import("$lib/projects")).store(),
});
