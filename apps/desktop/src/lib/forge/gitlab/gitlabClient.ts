import { Gitlab } from '@gitbeaker/rest';
import type { GitLabState } from '$lib/forge/gitlab/gitlabState.svelte';

type GitlabInstance = InstanceType<typeof Gitlab<false>>;

export class GitLabClient {
	api: GitlabInstance | undefined;
	forkProjectId: string | undefined;
	upstreamProjectId: string | undefined;
	instanceUrl: string | undefined;

	private callbacks: (() => void)[] = [];

	set(gitlabState: GitLabState) {
		this.forkProjectId = gitlabState.forkProjectId.current;
		this.upstreamProjectId = gitlabState.upstreamProjectId.current;
		if (gitlabState.token.current && gitlabState.instanceUrl.current) {
			this.api = new Gitlab({
				token: gitlabState.token.current,
				host: gitlabState.instanceUrl.current
			});
		} else {
			this.api = undefined;
		}
		this.callbacks.every((cb) => cb());
	}

	onReset(fn: () => void) {
		this.callbacks.push(fn);
		return () => (this.callbacks = this.callbacks.filter((cb) => cb !== fn));
	}
}

export function gitlab(extra: unknown): {
	api: GitlabInstance;
	forkProjectId: string;
	upstreamProjectId: string;
} {
	if (!hasGitLab(extra)) throw new Error('No GitHub client!');
	if (!extra.gitLabClient.api) throw new Error('Things are sad');
	if (!extra.gitLabClient.forkProjectId) throw new Error('Things are sad');
	if (!extra.gitLabClient.upstreamProjectId) throw new Error('Things are sad');

	// Equivalent to using the readable's `get` function
	return {
		api: extra.gitLabClient.api!,
		forkProjectId: extra.gitLabClient.forkProjectId,
		upstreamProjectId: extra.gitLabClient.upstreamProjectId
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
