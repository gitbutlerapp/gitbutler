import type { PageLoad } from "./$types";
import { derived } from "svelte/store";
import { building } from "$app/environment";

export const prerender = false;
export const load: PageLoad = async ({ parent, params }) => {
    const { sessions } = await parent();
    const deltas = building
        ? []
        : (await import("$lib/deltas")).list({
              projectId: params.id,
              sessionId: params.hash,
          });
    const files = building
        ? ({} as Record<string, string>)
        : (await import("$lib/sessions")).listFiles({
              projectId: params.id,
              sessionId: params.hash,
          });
    const session = derived(sessions, (sessions) =>
        sessions.find((session) => session.hash === params.hash)
    );
    return {
        session,
        previousSesssion: derived(sessions, (sessions) => {
            const currentSessionIndex = sessions.findIndex(
                (session) => session.hash === params.hash
            );
            if (currentSessionIndex - 1 < sessions.length) {
                return sessions[currentSessionIndex - 1];
            } else {
                return undefined;
            }
        }),
        nextSession: derived(sessions, (sessions) => {
            const currentSessionIndex = sessions.findIndex(
                (session) => session.hash === params.hash
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
