import { AzureBranch } from '$lib/forge/azure/azureBranch';
import type { Forge, ForgeName } from '$lib/forge/interface/forge';
import type { ForgeRepoService } from '$lib/forge/interface/forgeRepoService';
import type { ForgeArguments } from '$lib/forge/interface/types';
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
		this.baseUrl = `https://${AZURE_DOMAIN}/${repo.organization}/${repo.owner}/_git/${repo.name}`;
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
