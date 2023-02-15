import { readable } from "svelte/store";
import type { LayoutLoad } from "./$types";
import { building } from "$app/environment";
import type { Project } from "$lib/projects";

export const ssr = false;
export const prerender = true;
export const csr = true;

export const load: LayoutLoad = async () => {
    const projects = building
        ? {
            ...readable<Project[]>([]),
            add: () => {
                throw new Error("not implemented");
            },
            update: () => {
                throw new Error("not implemented");
            },
        }
        : await (await import("$lib/projects")).default();
    const user = building
        ? {
            ...readable<undefined>(undefined),
            set: () => {
                throw new Error("not implemented");
            },
            delete: () => {
                throw new Error("not implemented");
            },
        }
        : await (await import("$lib/users")).default();
    return { projects, user };
};
