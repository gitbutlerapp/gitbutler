import { readable, writable } from "svelte/store";
import type { LayoutLoad } from "./$types";
import { building } from "$app/environment";
import type { Project } from "$lib/projects";

export const ssr = false;
export const prerender = true;
export const csr = true;

export const load: LayoutLoad = async () => {
    const projects = building ? ({
        ...readable<Project[]>([]),
        add: () => {
            throw new Error("not implemented");
        }
    }) : await (await import("$lib/projects")).default();
    const user = building ? writable<undefined>(undefined) : await (await import("$lib/users")).default();
    return { projects, user }
};
