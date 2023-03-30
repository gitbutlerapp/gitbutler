import { invoke } from '@tauri-apps/api';

export { default as statuses } from './statuses';
export { default as activity } from './activity';

export const commit = (params: {
    projectId: string;
    message: string;
    files: Array<string>;
    push: boolean;
}) => invoke<boolean>('git_commit', params);

export const stage = (params: { projectId: string; paths: Array<string> }) =>
    invoke<void>('git_stage', params);

export const unstage = (params: { projectId: string; paths: Array<string> }) =>
    invoke<void>('git_unstage', params);
