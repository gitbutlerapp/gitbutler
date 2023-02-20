import { readable, derived } from "svelte/store";
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
    const orderedSessions = derived(sessions, (sessions) => {
        return sessions.slice().sort((a, b) => a.meta.startTimestampMs - b.meta.startTimestampMs);
    });
    return {
        project: projects.get(params.projectId),
        sessions: orderedSessions,
    };
};
