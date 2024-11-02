import { AzureBranch } from './azureBranch';
import { type Forge } from '$lib/forge/interface/forge';
import { ForgeName, type ForgeArguments } from '$lib/forge/interface/types';
import type { RepoInfo } from '$lib/url/gitUrl';

export const AZURE_DOMAIN = 'dev.azure.com';

/**
 * PR support is pending OAuth support in the rust code.
 *
 * Follow this issue to stay in the loop:
 * https://github.com/gitbutlerapp/gitbutler/issues/2651
 */
export class AzureDevOps implements Forge {
	readonly name = ForgeName.Azure;
	private baseUrl: string;
	private repo: RepoInfo;
	private baseBranch: string;
	private forkStr?: string;

	constructor({ repo, baseBranch, forkStr }: ForgeArguments) {
		this.baseUrl = `https://${AZURE_DOMAIN}/${repo.organization}/${repo.owner}/_git/${repo.name}`;
		this.repo = repo;
		this.baseBranch = baseBranch;
		this.forkStr = forkStr;
	}

	branch(name: string) {
		return new AzureBranch(name, this.baseBranch, this.baseUrl, this.forkStr);
	}

	commitUrl(id: string): string {
		return `${this.baseUrl}/commit/${id}`;
	}

	listService() {
		return undefined;
	}

	issueService() {
		return undefined;
	}

	prService() {
		return undefined;
	}

	checksMonitor(_sourceBranch: string) {
		return undefined;
	}
}
