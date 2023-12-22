import { Octokit } from '@octokit/rest';

export function newClient(authToken: string) {
	return new Octokit({
		auth: authToken,
		userAgent: 'GitButler Client',
		baseUrl: 'https://api.github.com'
	});
}
