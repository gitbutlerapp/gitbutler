import { derived, readable } from "svelte/store";
import type { LayoutLoad } from "./$types";
import { building } from "$app/environment";
import type { Session } from "$lib/sessions";

export const prerender = false;

export const load: LayoutLoad = async ({ parent, params }) => {
    const { projects } = await parent();
    const sessions = building
        ? readable<Session[]>([])
        : await (
            await import("$lib/sessions")
        ).default({ projectId: params.projectId });
    return {
        project: derived(projects, (projects) =>
            projects.find((project) => project.id === params.projectId)
        ),
        sessions,
    };
};
