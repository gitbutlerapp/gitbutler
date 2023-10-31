import { asyncWritable, type WritableLoadable, type Loadable } from '@square/svelte-store';
import lscache from 'lscache';

import { User } from '$lib/github/types';
import { newClient } from '$lib/github/client';

// Uses the cached value as the initial state and also in the event of being offline
export function getAuthenticatedWithCache(ctx: { authToken: string }): Loadable<User> {
	const key = 'github/user';
	const store = asyncWritable(
		[],
		async () => lscache.get(key) || [],
		async (data) => data,
		{ trackState: true },
		(set) => {
			getAuthenticated(ctx).then((user) => {
				lscache.set(key, user, 1440 * 28); // 28 day ttl
				set(user);
			});
		}
	) as WritableLoadable<User>;
	return store;
}

export async function getAuthenticated(ctx: { authToken: string }): Promise<User> {
	const octokit = newClient(ctx);
	try {
		const rsp = await octokit.users.getAuthenticated();
		return new User(rsp.data.login, rsp.data.email || undefined, rsp.data.type === 'Bot');
	} catch (e) {
		console.log(e);
		throw e;
	}
}
