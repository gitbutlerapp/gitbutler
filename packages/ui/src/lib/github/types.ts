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
}
