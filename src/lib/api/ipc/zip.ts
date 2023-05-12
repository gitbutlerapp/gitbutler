import { invoke } from '$lib/ipc';

export const logs = () => invoke<string>('get_logs_archive_path');

export const gitbutlerData = (params: { projectId: string }) =>
	invoke<string>('get_project_data_archive_path', params);

export const projectData = (params: { projectId: string }) =>
	invoke<string>('get_project_archive_path', params);
