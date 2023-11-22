import { newClient } from '$lib/github/client';

export async function getAuthenticated(ctx: { authToken: string }): Promise<string> {
	const octokit = newClient(ctx);
	try {
		const rsp = await octokit.users.getAuthenticated();
		return rsp.data.login;
	} catch (e) {
		console.log(e);
		throw e;
	}
}
