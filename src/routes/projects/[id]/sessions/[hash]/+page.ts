import type { PageLoad } from "./$types";
// import type { Project } from "$lib/projects";
import { derived, readable } from "svelte/store";
import type { Session } from "$lib/session";

export const prerender = false;
export const load: PageLoad = async ({ parent, params }) => {
    const { sessions } = await parent();
    return {
        session: derived(sessions, (sessions) =>
            sessions.find((session: Session) => session.hash === params.hash)
        ),
    }
}
