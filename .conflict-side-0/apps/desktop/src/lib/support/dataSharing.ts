import { invoke } from '$lib/backend/ipc';

export async function logs() {
	return await invoke<string>('get_logs_archive_path');
}

export async function projectData(params: { projectId: string }) {
	return await invoke<string>('get_project_archive_path', params);
}
