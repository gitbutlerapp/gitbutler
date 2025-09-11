import { AzureBranch } from '$lib/forge/azure/azureBranch';
import type { Forge, ForgeName } from '$lib/forge/interface/forge';
import type { ForgeRepoService } from '$lib/forge/interface/forgeRepoService';
import type { ForgeArguments, ForgeUser } from '$lib/forge/interface/types';
import type { ReactiveResult } from '$lib/state/butlerModule';
import type { ReduxTag } from '$lib/state/tags';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { TagDescription } from '@reduxjs/toolkit/query';

export const AZURE_DOMAIN = 'dev.azure.com';

/**
 * PR support is pending OAuth support in the rust code.
 *
 * Follow this issue to stay in the loop:
 * https://github.com/gitbutlerapp/gitbutler/issues/2651
 */
export class AzureDevOps implements Forge {
	readonly name: ForgeName = 'azure';
	readonly authenticated: boolean;
	private baseUrl: string;
	private repo: RepoInfo;
	private baseBranch: string;
	private forkStr?: string;

	constructor({ repo, baseBranch, forkStr, authenticated }: ForgeArguments) {
		// Use the protocol from repo if available, otherwise default to https
		// For SSH remote URLs, always use HTTPS for browser compatibility
		let protocol = repo.protocol?.endsWith(':')
			? repo.protocol.slice(0, -1)
			: repo.protocol || 'https';

		// SSH URLs cannot be opened in browsers, so convert to HTTPS
		if (protocol === 'ssh') {
			protocol = 'https';
		}

		this.baseUrl = `${protocol}://${repo.domain}/${repo.organization}/${repo.owner}/_git/${repo.name}`;
		this.repo = repo;
		this.baseBranch = baseBranch;
		this.forkStr = forkStr;
		this.authenticated = authenticated;
	}

	branch(name: string) {
		return new AzureBranch(name, this.baseBranch, this.baseUrl, this.forkStr);
	}

	commitUrl(id: string): string {
		return `${this.baseUrl}/commit/${id}`;
	}

	get user() {
		return {
			current: { status: 'uninitialized' as const, data: undefined }
		} as ReactiveResult<ForgeUser>;
	}

	get listService() {
		return undefined;
	}

	get issueService() {
		return undefined;
	}

	get prService() {
		return undefined;
	}

	get repoService(): ForgeRepoService | undefined {
		return undefined;
	}

	get checks() {
		return undefined;
	}

	invalidate(_tags: TagDescription<ReduxTag>[]) {
		return undefined;
	}
}
