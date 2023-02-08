import type { Delta } from "$lib/crdt";
import { derived, readable } from "svelte/store";
// import type { PageLoad } from "./$types";
import type { LayoutLoad } from "./$types";
// import crdt from "$lib/crdt";
import { building } from "$app/environment";
import type { Session } from "$lib/session";

export const prerender = false;

export const load: LayoutLoad = async ({ parent, params }) => {
    const { projects } = await parent();
    const deltas = building
        ? readable<Record<string, Delta[]>>({})
        : await (await import("$lib/crdt")).default({ projectId: params.id })

    return {
        project: derived(projects, (projects) =>
            projects.find((project) => project.id === params.id)
        ),
        deltas,
        sessions: derived(deltas, (deltas) => {
            const files = Object.entries(deltas).map(([key, value]) => (
                {
                    name: key,
                    path: key, // TODO
                    linesTouched: 0, // TODO
                    numberOfEdits: 0, // TODO
                    deltas: value,
                }
            ))
            const infiniteSession: Session = {
                hash: "1-a1b2c3d4e5f6g7h8i9j0", // TODO: set this when we have a snapshot
                startTime: 0, // TODO: set this when we have a snapshot
                endTime: 0, // TODO: set this when we have a snapshot
                branchName: "infinite-session-x", // TODO: set this when we have a snapshot
                files: files,
                activities: [] // TODO: set this when we have activities (e.g. push, commit, etc.)
            }
            // TODO: until we have multiple snapshots, putting all crdt changes into one session
            return [infiniteSession]
        }),
    };
};
