import { InjectionToken } from '@gitbutler/core/context';
import { persisted } from '@gitbutler/shared/persisted';
import { derived, get, writable, type Readable, type Writable } from 'svelte/store';
import type { SecretsService } from '$lib/secrets/secretsService';
import type { RepoInfo } from '$lib/url/gitUrl';

export const GITLAB_STATE = new InjectionToken<GitLabState>('GitLabState');

export class GitLabState {
	readonly token: Writable<string | undefined>;
	readonly forkProjectId: Writable<string | undefined>;
	readonly upstreamProjectId: Writable<string | undefined>;
	readonly instanceUrl: Writable<string | undefined>;
	readonly configured: Readable<boolean>;

	constructor(
		private readonly secretService: SecretsService,
		repoInfo: RepoInfo | undefined,
		projectId: string
	) {
		// For whatever reason, the token _sometimes_ is incorrectly fetched as null.
		// I have no idea why, but this seems to work. There were also some
		// weird reactivity issues. Don't touch it as you might make it angry.
		const tokenLoading = writable(true);
		let tokenLoadedAsNull = false;
		this.token = writable<string | undefined>();
		this.secretService.get(`git-lab-token:${projectId}`).then((fetchedToken) => {
			if (fetchedToken) {
				this.token.set(fetchedToken ?? '');
			} else {
				tokenLoadedAsNull = true;
			}
			tokenLoading.set(false);
		});
		let _tokenUnsubscribe: (() => void) | undefined;

		const _loadingUnsubscribe = tokenLoading.subscribe((loading) => {
			if (loading) {
				return;
			}
			// Set up the token subscription to save changes
			_tokenUnsubscribe = this.token.subscribe((token) => {
				if (!token && tokenLoadedAsNull) {
					return;
				}
				this.secretService.set(`git-lab-token:${projectId}`, token ?? '');
				tokenLoadedAsNull = false;
			});
		});

		const forkProjectId = persisted<string | undefined>(
			undefined,
			`gitlab-project-id:${projectId}`
		);
		if (!get(forkProjectId) && repoInfo) {
			forkProjectId.set(`${repoInfo.owner}/${repoInfo.name}`);
		}
		this.forkProjectId = forkProjectId;

		const upstreamProjectId = persisted<string | undefined>(
			undefined,
			`gitlab-upstream-project-id:${projectId}`
		);
		if (!get(upstreamProjectId)) {
			upstreamProjectId.set(get(forkProjectId));
		}
		this.upstreamProjectId = upstreamProjectId;

		const instanceUrl = persisted<string>('https://gitlab.com', `gitlab-instance-url:${projectId}`);
		this.instanceUrl = instanceUrl;

		this.configured = derived(
			[this.token, this.upstreamProjectId, this.forkProjectId, this.instanceUrl],
			([token, upstreamProjectId, forkProjectId, instanceUrl]) => {
				return !!token && !!upstreamProjectId && !!forkProjectId && !!instanceUrl;
			}
		);
	}
}
