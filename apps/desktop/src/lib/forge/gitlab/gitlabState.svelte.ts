import { persisted } from '@gitbutler/shared/persisted';
import {
	reactive,
	writableReactive,
	writableToReactive
} from '@gitbutler/shared/reactiveUtils.svelte';
import { get } from 'svelte/store';
import type { SecretsService } from '$lib/secrets/secretsService';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { Reactive, WritableReactive } from '@gitbutler/shared/storeUtils';

export class GitLabState {
	readonly token: WritableReactive<string | undefined>;
	readonly forkProjectId: WritableReactive<string | undefined>;
	readonly upstreamProjectId: WritableReactive<string | undefined>;
	readonly instanceUrl: WritableReactive<string | undefined>;
	readonly configured: Reactive<boolean>;

	constructor(
		private readonly secretService: SecretsService,
		repoInfo: RepoInfo | undefined,
		projectId: string
	) {
		let token = $state<string>('');
		let tokenLoading = true;
		this.secretService.get(`git-lab-token:${projectId}`).then((fetchedToken) => {
			token = fetchedToken ?? '';
			tokenLoading = false;
		});
		$effect(() => {
			// eslint-disable-next-line @typescript-eslint/no-unused-expressions
			token; // Make sure we always try to react to token
			if (tokenLoading) return;
			this.secretService.set(`git-lab-token:${projectId}`, token ?? '');
		});
		this.token = writableReactive(
			() => token,
			(value) => (token = value)
		);

		const forkProjectId = persisted<string | undefined>(
			undefined,
			`gitlab-project-id:${projectId}`
		);
		if (!get(forkProjectId) && repoInfo) {
			forkProjectId.set(`${repoInfo.owner}/${repoInfo.name}`);
		}
		this.forkProjectId = writableToReactive(forkProjectId);

		const upstreamProjectId = persisted<string | undefined>(
			undefined,
			`gitlab-upstream-project-id:${projectId}`
		);
		if (!get(upstreamProjectId)) {
			upstreamProjectId.set(get(forkProjectId));
		}
		this.upstreamProjectId = writableToReactive(upstreamProjectId);

		const instanceUrl = persisted<string>('https://gitlab.com', `gitlab-instance-url:${projectId}`);
		this.instanceUrl = writableToReactive(instanceUrl);

		const configured = $derived(!!this.token.current && !!this.forkProjectId && !!this.instanceUrl);
		this.configured = reactive(() => configured);
	}
}
