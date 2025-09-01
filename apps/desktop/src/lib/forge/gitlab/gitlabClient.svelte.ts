import { Gitlab } from '@gitbeaker/rest';
import { InjectionToken } from '@gitbutler/core/context';
import { derived } from 'svelte/store';
import type { GitLabState } from '$lib/forge/gitlab/gitlabState.svelte';

type GitlabInstance = InstanceType<typeof Gitlab<false>>;

export const GITLAB_CLIENT = new InjectionToken<GitLabClient>('GitLabClient');

export class GitLabClient {
	api: GitlabInstance | undefined;
	forkProjectId: string | undefined;
	upstreamProjectId: string | undefined;
	instanceUrl: string | undefined;

	private callbacks: (() => void)[] = [];

	set(gitlabState: GitLabState) {
		const subscribable = derived(
			[
				gitlabState.forkProjectId,
				gitlabState.upstreamProjectId,
				gitlabState.instanceUrl,
				gitlabState.token
			],
			([forkProjectId, upstreamProjectId, instanceUrl, token]) => {
				this.forkProjectId = forkProjectId;
				this.upstreamProjectId = upstreamProjectId;
				if (token && instanceUrl) {
					this.api = new Gitlab({
						token,
						host: instanceUrl
					});
				} else {
					this.api = undefined;
				}
				this.callbacks.every((cb) => cb());
			}
		);

		$effect(() => {
			const unsubscribe = subscribable.subscribe(() => {});
			return unsubscribe;
		});
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
	if (!extra.gitLabClient.api) throw new Error('Failed to find GitLab client');
	if (!extra.gitLabClient.forkProjectId) throw new Error('Failed to find fork project ID');
	if (!extra.gitLabClient.upstreamProjectId) throw new Error('Failed to find upstream project ID');

	// Equivalent to using the readable's `get` function
	return {
		api: extra.gitLabClient.api!,
		forkProjectId: extra.gitLabClient.forkProjectId,
		upstreamProjectId: extra.gitLabClient.upstreamProjectId
	};
}

function hasGitLab(extra: unknown): extra is {
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
