import type { User } from '../cloud/api';
import { invoke } from '$lib/ipc';

export async function get() {
	return invoke<User | null>('get_user');
}

export async function set(params: { user: User }) {
	return invoke<User>('set_user', params);
}

const del = () => invoke<void>('delete_user');
export { del as delete };
