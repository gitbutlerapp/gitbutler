import { InjectionToken } from '@gitbutler/core/context';
import { persisted } from '@gitbutler/shared/persisted';
import { derived, get, readable, writable, type Readable, type Writable } from 'svelte/store';
import type { SecretsService } from '$lib/secrets/secretsService';
import type { RepoInfo } from '$lib/url/gitUrl';

export const GITLAB_STATE = new InjectionToken<GitLabState>('GitLabState');

export class GitLabState {
	readonly token: Writable<string | undefined>;
	private _forkProjectId: Writable<string | undefined> | undefined;
	private _upstreamProjectId: Writable<string | undefined> | undefined;
	private _instanceUrl: Writable<string | undefined> | undefined;
	private _configured: Readable<boolean> | undefined;
	private _currentProjectId: string | undefined;
	private _unsubscribers: Array<() => void> = [];

	constructor(private readonly secretService: SecretsService) {
		this.token = writable<string | undefined>();
	}

	get forkProjectId() {
		if (!this._forkProjectId) {
			return writable<string | undefined>(undefined);
		}
		return this._forkProjectId;
	}

	get upstreamProjectId() {
		if (!this._upstreamProjectId) {
			return writable<string | undefined>(undefined);
		}
		return this._upstreamProjectId;
	}

	get instanceUrl() {
		if (!this._instanceUrl) {
			return writable<string | undefined>(undefined);
		}
		return this._instanceUrl;
	}

	get configured() {
		if (!this._configured) {
			return readable(false);
		}
		return this._configured;
	}

	init(projectId: string, repoInfo: RepoInfo | undefined) {
		// Skip if already initialized for this project
		if (this._currentProjectId === projectId) {
			return;
		}

		// Clean up previous subscriptions before reinitializing
		this.cleanup();

		this._currentProjectId = projectId;

		// Track initial load to prevent saving the token we just loaded
		let isInitialLoad = true;
		let tokenLoadedAsNull = false;

		// Load token from secret service
		this.secretService.get(`git-lab-token:${projectId}`).then((fetchedToken) => {
			if (fetchedToken) {
				this.token.set(fetchedToken ?? '');
			} else {
				tokenLoadedAsNull = true;
			}
			isInitialLoad = false;
		});

		// Subscribe to token changes to sync with secret service
		const tokenUnsub = this.token.subscribe((token) => {
			// Skip during initial load to avoid redundant save
			if (isInitialLoad) {
				return;
			}

			// Skip if token was loaded as null and is still null
			if (!token && tokenLoadedAsNull) {
				return;
			}

			this.secretService.set(`git-lab-token:${projectId}`, token ?? '');
			tokenLoadedAsNull = false;
		});

		this._unsubscribers.push(tokenUnsub);

		this._forkProjectId = persisted<string | undefined>(
			undefined,
			`gitlab-project-id:${projectId}`
		);

		this._upstreamProjectId = persisted<string | undefined>(
			undefined,
			`gitlab-upstream-project-id:${projectId}`
		);
		if (!get(this._upstreamProjectId)) {
			this._upstreamProjectId.set(get(this._forkProjectId));
		}

		this._instanceUrl = persisted<string>('https://gitlab.com', `gitlab-instance-url:${projectId}`);

		this._configured = derived(
			[this.token, this.upstreamProjectId, this.forkProjectId, this.instanceUrl],
			([token, upstreamProjectId, forkProjectId, instanceUrl]) => {
				return !!token && !!upstreamProjectId && !!forkProjectId && !!instanceUrl;
			}
		);

		if (!get(this.forkProjectId) && repoInfo) {
			this.forkProjectId.set(`${repoInfo.owner}/${repoInfo.name}`);
		}
	}

	cleanup() {
		this._unsubscribers.forEach((unsub) => unsub());
		this._unsubscribers = [];
		this._currentProjectId = undefined;
	}
}
