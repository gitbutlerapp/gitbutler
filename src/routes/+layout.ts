import { readable } from "svelte/store";
import type { LayoutLoad } from "./$types";
import { building } from "$app/environment";
import type { Project } from "$lib/projects";

export const ssr = false;
export const prerender = true;
export const csr = true;

export const load: LayoutLoad = async () => {
    // tauri apis require window reference which doesn't exist during ssr, so we do not import it here.
    if (building) {
        return {
            projects: {
                ...readable<Project[]>([]),
                add: () => {
                    throw new Error("not implemented");
                },
            },
        };
    } else {
        const Projects = await import("$lib/projects");
        return {
            projects: await Projects.default(),
        };
    }
};
