import type { User } from '$lib/api';
import { invoke } from '$lib/ipc';
import { asyncWritable } from '@square/svelte-store';

export const get = async () => invoke<User | null>('get_user');

export const set = (params: { user: User }) => invoke<void>('set_user', params);

export const del = () => invoke<void>('delete_user');

export const CurrentUser = () => {
	const store = asyncWritable([], get);
	return {
		...store,
		set: async (user: User) => {
			await set({ user });
			store.set(user);
		},
		delete: async () => {
			await del();
			store.set(null);
		}
	};
};
