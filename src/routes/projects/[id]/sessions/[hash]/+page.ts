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
    return {
        session: derived(sessions, (sessions) =>
            sessions.find((session) => session.hash === params.hash)
        ),
        deltas,
        files,
    };
};
