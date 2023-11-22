import type { User } from './cloud';
import { invoke } from '$lib/backend/ipc';

export async function get() {
	return invoke<User | undefined>('get_user');
}

export async function set(params: { user: User }) {
	return invoke<User>('set_user', params);
}

export async function setCurrentProject(params: { projectId: string | undefined }) {
	return invoke<void>('set_current_project', params);
}

export async function getCurrentProject() {
	return invoke<string | undefined>('get_current_project');
}

const del = () => invoke<void>('delete_user');
export { del as delete };
