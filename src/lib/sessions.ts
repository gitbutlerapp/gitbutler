import { invoke } from "@tauri-apps/api";
import { appWindow } from "@tauri-apps/api/window";
import { writable } from "svelte/store";
import { log } from "$lib";

export type Activity = {
    type: string;
    timestampMs: number;
    message: string;
};

export type Session = {
    id: string;
    hash?: string;
    meta: {
        startTimestampMs: number;
        lastTimestampMs: number;
        branch: string;
        commit: string;
    };
    activity: Activity[];
};

export const listFiles = (params: { projectId: string; sessionId: string }) =>
    invoke<Record<string, string>>("list_session_files", params);

const list = (params: { projectId: string }) =>
    invoke<Session[]>("list_sessions", params);

export default async (params: { projectId: string }) => {
    const init = await list(params);
    const store = writable(init);
    const eventName = `project://${params.projectId}/sessions`;

    await appWindow.listen<Session>(eventName, (event) => {
        log.info(`Received sessions event ${eventName}`);
        store.update((sessions) => {
            const index = sessions.findIndex(
                (session) => session.id === event.payload.id
            );
            if (index === -1) {
                return [...sessions, event.payload];
            } else {
                return [
                    ...sessions.slice(0, index),
                    event.payload,
                    ...sessions.slice(index + 1),
                ];
            }
        });
    });

    return {
        subscribe: store.subscribe,
    };
};
