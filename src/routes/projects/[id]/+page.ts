import { derived } from "svelte/store";
import type { PageLoad } from "./$types";

export const prerender = false;

export const load: PageLoad = async ({ parent, params }) => {
    const { projects } = await parent();
    return {
        project: derived(projects, (projects) =>
            projects.find((project) => project.id === params.id)
        ),
    };
};
