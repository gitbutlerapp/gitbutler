import { InjectionToken } from '@gitbutler/core/context';
import { persisted } from '@gitbutler/shared/persisted';
import { derived, get, readable, writable, type Readable, type Writable } from 'svelte/store';
import type { SecretsService } from '$lib/secrets/secretsService';
import type { RepoInfo } from '$lib/url/gitUrl';

export const GITEA_STATE = new InjectionToken<GiteaState>('GiteaState');

export class GiteaState {
	readonly token: Writable<string | undefined>;
	private _instanceUrl: Writable<string | undefined> | undefined;
	private _configured: Readable<boolean> | undefined;
	private _currentProjectId: string | undefined;
	private _unsubscribers: Array<() => void> = [];

	constructor(private readonly secretService: SecretsService) {
		this.token = writable<string | undefined>();
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
		if (this._currentProjectId === projectId) {
			return;
		}

		this.cleanup();

		this._currentProjectId = projectId;

		let isInitialLoad = true;
		let tokenLoadedAsNull = false;

		this.secretService.get(`gitea-token:${projectId}`).then((fetchedToken) => {
			if (fetchedToken) {
				this.token.set(fetchedToken ?? '');
			} else {
				tokenLoadedAsNull = true;
			}
			isInitialLoad = false;
		});

		const tokenUnsub = this.token.subscribe((token) => {
			if (isInitialLoad) {
				return;
			}

			if (!token && tokenLoadedAsNull) {
				return;
			}

			this.secretService.set(`gitea-token:${projectId}`, token ?? '');
			tokenLoadedAsNull = false;
		});

		this._unsubscribers.push(tokenUnsub);

		// Default instance URL based on detected repo domain or codeberg.org
		const defaultUrl =
			repoInfo?.domain && repoInfo.domain !== 'github.com' && repoInfo.domain !== 'gitlab.com'
				? `https://${repoInfo.domain}`
				: 'https://codeberg.org';

		this._instanceUrl = persisted<string>(defaultUrl, `gitea-instance-url:${projectId}`);

		this._configured = derived([this.token, this.instanceUrl], ([token, instanceUrl]) => {
			return !!token && !!instanceUrl;
		});
	}

	cleanup() {
		this._unsubscribers.forEach((unsub) => unsub());
		this._unsubscribers = [];
		this._currentProjectId = undefined;
	}
}
