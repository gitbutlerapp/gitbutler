import { Octokit } from '@octokit/rest';

export function octokitFromAccessToken(accessToken: string | undefined) {
	return new Octokit({
		auth: accessToken,
		userAgent: 'GitButler Client',
		baseUrl: 'https://api.github.com'
	});
}
