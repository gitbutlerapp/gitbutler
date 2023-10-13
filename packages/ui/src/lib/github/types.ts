import { Octokit } from '@octokit/rest';

export type GitHubIntegrationContext = {
	authToken: string;
	owner: string;
    repo: string;
};

export type PullRequest = {
	html_url: string;
}
