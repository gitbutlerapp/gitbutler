import * as users from '$lib/api/ipc/users';
import { asyncWritable } from '@square/svelte-store';

export const userStore = asyncWritable([], users.get, async (user) => {
	if (!user) {
		await users.delete();
	} else {
		return await users.set({ user });
	}
});
