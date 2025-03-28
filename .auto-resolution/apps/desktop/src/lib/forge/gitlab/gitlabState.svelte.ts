import { persisted } from '@gitbutler/shared/persisted';
import {
	reactive,
	writableReactive,
	writableToReactive
} from '@gitbutler/shared/reactiveUtils.svelte';
import type { SecretsService } from '$lib/secrets/secretsService';
import type { Reactive, WritableReactive } from '@gitbutler/shared/storeUtils';

export class GitLabState {
	readonly token: WritableReactive<string | undefined>;
	readonly gitlabProjectId: WritableReactive<string | undefined>;
	readonly instanceUrl: WritableReactive<string | undefined>;
	readonly configured: Reactive<boolean>;

	constructor(
		private readonly secretService: SecretsService,
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

		const gitlabProjectId = persisted<string | undefined>(
			undefined,
			`gitlab-project-id:${projectId}`
		);
		const instanceUrl = persisted<string>('https://gitlab.com', `gitlab-instance-url:${projectId}`);

		this.token = writableReactive(
			() => token,
			(value) => (token = value)
		);
		this.gitlabProjectId = writableToReactive(gitlabProjectId);
		this.instanceUrl = writableToReactive(instanceUrl);

		const configured = $derived(
			!!this.token.current && !!this.gitlabProjectId && !!this.instanceUrl
		);
		this.configured = reactive(() => configured);
	}
}
