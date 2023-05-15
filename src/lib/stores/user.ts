import { users } from '$lib/api';
import { asyncWritable } from '@square/svelte-store';

const store = asyncWritable([], users.get, async (user) => {
	if (user === null) {
		await users.delete();
	} else {
		await users.set({ user });
	}
	return user;
});

export default store;
