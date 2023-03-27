import { building } from '$app/environment';
import type { Session } from '$lib/sessions';
import { readable, type Readable } from 'svelte/store';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ params }) => {
    const sessions: Readable<Session[]> = building
        ? readable<Session[]>([])
        : await import('$lib/sessions').then((m) => m.default({ projectId: params.projectId }));
    return {
        sessions,
        projectId: params.projectId
    };
};
