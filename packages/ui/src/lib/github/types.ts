import type { RestEndpointMethodTypes } from '@octokit/rest';

export type GitHubIntegrationContext = {
	authToken: string;
	owner: string;
	repo: string;
};

export class Label {
	constructor(
		public name: string,
		public description: string | undefined,
		public color: string
	) {}
}

export class User {
	constructor(
		public username: string,
		public email: string | undefined,
		public is_bot: boolean
	) {}

	static fromApi(
		user:
			| RestEndpointMethodTypes['users']['getAuthenticated']['response']['data']
			| RestEndpointMethodTypes['pulls']['create']['response']['data']['user']
	): User {
		return new User(user.login, user.email || undefined, user.type === 'Bot');
	}
}

export class PullRequest {
	constructor(
		public html_url: string,
		public number: number,
		public title: string,
		public body: string | undefined,
		public author: User | undefined,
		public labels: Label[],
		public draft: boolean,
		public created_at: string,
		public branch_name: string,
		public target_branch_name: string
	) {}

	static fromApi(
		pr:
			| RestEndpointMethodTypes['pulls']['create']['response']['data']
			| RestEndpointMethodTypes['pulls']['list']['response']['data'][number]
	): PullRequest {
		const author = pr.user ? User.fromApi(pr.user) : undefined;
		const labels = pr.labels.map((label) => {
			return new Label(label.name, label.description || undefined, label.color);
		});

		return new PullRequest(
			pr.html_url,
			pr.number,
			pr.title,
			pr.body || undefined,
			author,
			labels,
			pr.draft || false,
			pr.created_at,
			pr.head.ref,
			pr.base.ref
		);
	}
}
