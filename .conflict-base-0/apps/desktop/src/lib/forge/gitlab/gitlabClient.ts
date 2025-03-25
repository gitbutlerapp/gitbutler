import { Gitlab } from '@gitbeaker/rest';

type GitlabInstance = InstanceType<typeof Gitlab<false>>;

export class GitLabClient {
	api: GitlabInstance | undefined;
	projectId: string | undefined;
	instanceUrl: string | undefined;

	set(projectId?: string, token?: string, instanceUrl?: string) {
		this.projectId = projectId;
		if (token && instanceUrl) {
			this.api = new Gitlab({ token, host: instanceUrl });
		} else {
			this.api = undefined;
		}
	}
}

export function gitlab(extra: unknown): { api: GitlabInstance; projectId: string } {
	if (!hasGitLab(extra)) throw new Error('No GitHub client!');
	if (!extra.gitLabClient.api) throw new Error('Things are sad');
	if (!extra.gitLabClient.projectId) throw new Error('Things are sad');

	// Equivalent to using the readable's `get` function
	return {
		api: extra.gitLabClient.api!,
		projectId: extra.gitLabClient.projectId
	};
}

export function hasGitLab(extra: unknown): extra is {
	gitLabClient: GitLabClient;
} {
	return (
		!!extra &&
		typeof extra === 'object' &&
		extra !== null &&
		'gitLabClient' in extra &&
		extra.gitLabClient instanceof GitLabClient
	);
}
