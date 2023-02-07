import { derived } from "svelte/store";
import type { PageLoad } from "./$types";
import crdt from "$lib/crdt";

export const prerender = false;

export const load: PageLoad = async ({ parent, params }) => {
    const { projects } = await parent();
    return {
        project: derived(projects, (projects) =>
            projects.find((project) => project.id === params.id)
        ),
        deltas: await crdt({ projectId: params.id }),
    };
};
