import type { PageLoad } from "./$types";
import { derived, readable } from "svelte/store";
import { building } from "$app/environment";
import type { Delta } from "$lib/deltas";

export const prerender = false;
export const load: PageLoad = async ({ parent, params }) => {
    const { sessions } = await parent();
    const deltas = building
        ? readable({} as Record<string, Delta[]>)
        : (await import("$lib/deltas")).default({
              projectId: params.projectId,
              sessionId: params.sessionId,
          });
    console.log('HERE_0', params.projectId, params.sessionId)
    const files = building
        ? ({} as Record<string, string>)
        : (await import("$lib/sessions")).listFiles({
              projectId: params.projectId,
              sessionId: params.sessionId,
          });
    return {
        session: derived(sessions, (sessions) =>
            sessions.find((session) => session.id === params.sessionId)
        ),
        previousSesssion: derived(sessions, (sessions) => {
            const currentSessionIndex = sessions.findIndex(
                (session) => session.id === params.sessionId
            );
            if (currentSessionIndex - 1 < sessions.length) {
                return sessions[currentSessionIndex - 1];
            } else {
                return undefined;
            }
        }),
        nextSession: derived(sessions, (sessions) => {
            const currentSessionIndex = sessions.findIndex(
                (session) => session.id === params.sessionId
            );
            if (currentSessionIndex + 1 < sessions.length) {
                return sessions[currentSessionIndex + 1];
            } else {
                return undefined;
            }
        }),
        deltas,
        files,
    };
};
