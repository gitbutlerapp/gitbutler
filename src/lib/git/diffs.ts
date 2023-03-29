import { invoke } from '@tauri-apps/api';
import { appWindow } from '@tauri-apps/api/window';
import { writable, type Readable } from 'svelte/store';
import { log } from '$lib';

const getDiffs = (params: { projectId: string }) =>
    invoke<Record<string, string>>('git_wd_diff', params);

export default async (params: { projectId: string }) => {
    const diffs = await getDiffs(params);
    const store = writable(diffs);

    appWindow.listen(`project://${params.projectId}/sessions`, async () => {
        log.info(`Status: Received sessions event, projectId: ${params.projectId}`);
        store.set(await getDiffs(params));
    });

    appWindow.listen(`project://${params.projectId}/git/index`, async () => {
        log.info(`Status: Received git activity event, projectId: ${params.projectId}`);
        store.set(await getDiffs(params));
    });

    return store as Readable<Record<string, string>>;
};
