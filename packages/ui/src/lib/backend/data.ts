import { invoke } from '$lib/backend/ipc';

export function deleteAllData() {
	invoke<void>('delete_all_data');
}
