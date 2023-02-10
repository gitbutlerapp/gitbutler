import { invoke } from "@tauri-apps/api";
import { appWindow } from "@tauri-apps/api/window";
import { writable } from "svelte/store";

export type Session = {
    hash: string;
    meta: {
        startTs: number;
        lastTs: number;
        branch: string;
        commit: string;
    };
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
        store.update((sessions) => [...sessions, event.payload]);
    });

    return {
        subscribe: store.subscribe,
    };
};
