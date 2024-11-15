import { rateLimit } from '$lib/utils/ratelimit';
import { Octokit } from '@octokit/rest';

export function octokitFromAccessToken(accessToken: string) {
	return new Octokit({
		auth: accessToken,
		userAgent: 'GitButler Client',
		baseUrl: 'https://api.github.com',
		request: {
			// Global rate-limiter to mitigate accidental reactivity bugs that
			// could trigger runaway requests.
			fetch: rateLimit({
				name: 'Octokit',
				limit: 100,
				windowMs: 60 * 1000,
				fn: async (input: RequestInfo | URL, init?: RequestInit) => {
					return await window.fetch(input, init);
				}
			})
		}
	});
}
