import { Octokit } from '@octokit/rest';

export function newClient(ctx: { authToken: string }) {
	return new Octokit({
		auth: ctx.authToken,
		userAgent: 'GitButler Client',
		baseUrl: 'https://api.github.com'
	});
}
