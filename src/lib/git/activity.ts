import { invoke } from '@tauri-apps/api';
import { appWindow } from '@tauri-apps/api/window';
import { writable, type Readable } from 'svelte/store';
import { log } from '$lib';

export type Activity = {
    type: string;
    timestampMs: number;
    message: string;
};

const list = (params: { projectId: string; startTimeMs?: number }) =>
    invoke<Activity[]>('git_activity', params);

export default async (params: { projectId: string }) => {
    const activity = await list(params);
    const store = writable(activity);

    appWindow.listen(`project://${params.projectId}/git/activity`, async () => {
        log.info(`Status: Received git activity event, projectId: ${params.projectId}`);
        const startTimeMs = activity.at(-1)?.timestampMs;
        const newActivities = await list({ projectId: params.projectId, startTimeMs });
        store.update((activities) => [...activities, ...newActivities]);
    });

    return store as Readable<Activity[]>;
};
