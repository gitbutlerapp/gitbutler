import { User } from '$lib/github/types';
import { newClient } from '$lib/github/client';

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
