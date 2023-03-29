import { invoke } from '@tauri-apps/api';
import { appWindow } from '@tauri-apps/api/window';
import { writable, type Readable } from 'svelte/store';
import { log } from '$lib';
import type { Activity } from './git/activity';

export namespace Session {
    export const within = (session: Session | undefined, timestampMs: number) => {
        if (!session) return false;
        const { startTimestampMs, lastTimestampMs } = session.meta;
        return startTimestampMs <= timestampMs && timestampMs <= lastTimestampMs;
    };
}

export type Session = {
    id: string;
    hash?: string;
    meta: {
        startTimestampMs: number;
        lastTimestampMs: number;
        branch?: string;
        commit?: string;
    };
    activity: Activity[];
};

const filesCache: Record<string, Record<string, Promise<Record<string, string>>>> = {};

export const listFiles = async (params: {
    projectId: string;
    sessionId: string;
    paths?: string[];
}) => {
    const sessionFilesCache = filesCache[params.projectId] || {};
    if (params.sessionId in sessionFilesCache) {
        return sessionFilesCache[params.sessionId].then((files) => {
            return Object.fromEntries(
                Object.entries(files).filter(([path]) =>
                    params.paths ? params.paths.includes(path) : true
                )
            );
        });
    }

    const promise = invoke<Record<string, string>>('list_session_files', {
        sessionId: params.sessionId,
        projectId: params.projectId
    });
    sessionFilesCache[params.sessionId] = promise;
    filesCache[params.projectId] = sessionFilesCache;
    return promise.then((files) => {
        return Object.fromEntries(
            Object.entries(files).filter(([path]) => (params.paths ? params.paths.includes(path) : true))
        );
    });
};

const sessionsCache: Record<string, Promise<Session[]>> = {};

const list = async (params: { projectId: string; earliestTimestampMs?: number }) => {
    if (params.projectId in sessionsCache) {
        return sessionsCache[params.projectId].then((sessions) =>
            sessions.filter((s) =>
                params.earliestTimestampMs ? s.meta.startTimestampMs >= params.earliestTimestampMs : true
            )
        );
    }
    sessionsCache[params.projectId] = invoke<Session[]>('list_sessions', {
        projectId: params.projectId
    });
    return sessionsCache[params.projectId].then((sessions) =>
        sessions.filter((s) =>
            params.earliestTimestampMs ? s.meta.startTimestampMs >= params.earliestTimestampMs : true
        )
    );
};

export default async (params: { projectId: string; earliestTimestampMs?: number }) => {
    const store = writable([] as Session[]);
    list(params).then((sessions) => {
        store.set(sessions);
    });

    appWindow.listen<Session>(`project://${params.projectId}/sessions`, async (event) => {
        log.info(`Received sessions event, projectId: ${params.projectId}`);
        const session = event.payload;
        store.update((sessions) => {
            const index = sessions.findIndex((session) => session.id === event.payload.id);
            if (index === -1) {
                return [...sessions, session];
            } else {
                return [...sessions.slice(0, index), session, ...sessions.slice(index + 1)];
            }
        });
    });

    return store as Readable<Session[]>;
};
