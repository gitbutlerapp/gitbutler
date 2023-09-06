import { invoke } from '$lib/ipc';

export function deleteAllData() {
	invoke<void>('delete_all_data');
}
