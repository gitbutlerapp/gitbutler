import type { User } from '$lib/api';
import { invoke } from '$lib/ipc';
import { writable } from 'svelte/store';

export const get = () => invoke<User | undefined>('get_user');

export const set = (params: { user: User }) => invoke<void>('set_user', params);

export const del = () => invoke<void>('delete_user');

export const CurrentUser = async () => {
	const store = writable<User | undefined>(await get());
	return {
		subscribe: store.subscribe,
		set: async (user: User) => {
			await set({ user });
			store.set(user);
		},
		delete: async () => {
			await del();
			store.set(undefined);
		}
	};
};
