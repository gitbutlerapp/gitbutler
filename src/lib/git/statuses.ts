import { invoke } from '@tauri-apps/api';
import { appWindow } from '@tauri-apps/api/window';
import { writable, type Readable } from 'svelte/store';

export type Status = {
    path: string;
    status: FileStatus;
    staged: boolean;
};

type FileStatus = 'added' | 'modified' | 'deleted' | 'renamed' | 'typeChange' | 'other';

const list = (params: { projectId: string }) =>
    invoke<Record<string, [FileStatus, boolean]>>('git_status', params);

const convertToStatuses = (statusesGit: Record<string, [FileStatus, boolean]>): Status[] =>
    Object.entries(statusesGit).map((status) => ({
        path: status[0],
        status: status[1][0],
        staged: status[1][1]
    }));

export default async (params: { projectId: string }) => {
    const statuses = await list(params).then(convertToStatuses);
    const store = writable(statuses);

    [
        `project://${params.projectId}/git/index`,
        `project://${params.projectId}/git/activity`,
        `project://${params.projectId}/sessions`
    ].forEach((eventName) => {
        appWindow.listen(eventName, async () => {
            store.set(await list(params).then(convertToStatuses));
        });
    });

    return store as Readable<Status[]>;
};
