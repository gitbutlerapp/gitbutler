import { giteaApi } from 'gitea-js';
import { derived } from 'svelte/store';
import type { GiteaState } from '$lib/forge/gitea/giteaState.svelte';
import { isValidGiteaProjectId, type GiteaProjectId } from '$lib/forge/gitea/types';

type GiteaInstance = ReturnType<typeof giteaApi>;

export class GiteaClient {
	api: GiteaInstance | undefined;
	forkProjectId: GiteaProjectId | undefined;
	upstreamProjectId: GiteaProjectId | undefined;
	instanceUrl: string | undefined;

	private callbacks: (() => void)[] = [];

	set(giteaState: GiteaState) {
		const subscribable = derived(
			[
				giteaState.forkProjectId,
				giteaState.upstreamProjectId,
				giteaState.instanceUrl,
				giteaState.token
			],
			([forkProjectId, upstreamProjectId, instanceUrl, token]) => {
				if (!isValidGiteaProjectId(forkProjectId) || !isValidGiteaProjectId(upstreamProjectId)) {
					throw new Error('Invalid Gitea project ID');
				}

				this.forkProjectId = forkProjectId;
				this.upstreamProjectId = upstreamProjectId;
				if (token && instanceUrl) {
					this.api = giteaApi(instanceUrl, { token });
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

export function gitea(extra: unknown): {
	api: GiteaInstance;
	forkProjectId: GiteaProjectId;
	upstreamProjectId: GiteaProjectId;
} {
	if (!hasGitea(extra)) throw new Error('No Gitea client!');
	if (!extra.giteaClient.api) throw new Error('Failed to find Gitea client');
	if (!extra.giteaClient.forkProjectId) throw new Error('Failed to find fork project ID');
	if (!extra.giteaClient.upstreamProjectId) throw new Error('Failed to find upstream project ID');

	// Equivalent to using the readable's `get` function
	return {
		api: extra.giteaClient.api!,
		forkProjectId: extra.giteaClient.forkProjectId,
		upstreamProjectId: extra.giteaClient.upstreamProjectId
	};
}

export function hasGitea(extra: unknown): extra is {
	giteaClient: GiteaClient;
} {
	return (
		!!extra &&
		typeof extra === 'object' &&
		extra !== null &&
		'giteaClient' in extra &&
		extra.giteaClient instanceof GiteaClient
	);
}
