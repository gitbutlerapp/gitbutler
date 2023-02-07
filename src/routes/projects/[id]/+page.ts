import type { Delta } from "$lib/crdt";
import { derived, readable } from "svelte/store";
import type { PageLoad } from "./$types";
// import crdt from "$lib/crdt";
import { building } from "$app/environment";

export const prerender = false;

export const load: PageLoad = async ({ parent, params }) => {
    const { projects } = await parent();
    return {
        project: derived(projects, (projects) =>
            projects.find((project) => project.id === params.id)
        ),
        deltas: building
            ? readable<Record<string, Delta[]>>({})
            : (await import("$lib/crdt")).default({ projectId: params.id }),
    };
};
