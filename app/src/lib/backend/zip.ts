import { invoke } from '$lib/backend/ipc';

export function logs() {
	return invoke<string>('get_logs_archive_path');
}

export function gitbutlerData(params: { projectId: string }) {
	return invoke<string>('get_project_data_archive_path', params);
}

export function projectData(params: { projectId: string }) {
	return invoke<string>('get_project_archive_path', params);
}
