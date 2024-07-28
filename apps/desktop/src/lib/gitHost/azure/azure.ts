import { AzureBranch } from './azureBranch';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { GitHost } from '../interface/gitHost';

export const AZURE_DOMAIN = 'dev.azure.com';

/**
 * PR support is pending OAuth support in the rust code.
 *
 * Follow this issue to stay in the loop:
 * https://github.com/gitbutlerapp/gitbutler/issues/2651
 */
export class AzureDevOps implements GitHost {
	url: string;

	constructor(
		repo: RepoInfo,
		private baseBranch: string,
		private fork?: string
	) {
		this.url = `https://${AZURE_DOMAIN}/${repo.organization}/${repo.owner}/_git/${repo.name}`;
	}

	branch(name: string) {
		return new AzureBranch(name, this.baseBranch, this.url, this.fork);
	}

	commitUrl(id: string): string {
		return `${this.url}/commit/${id}`;
	}

	listService() {
		return undefined;
	}

	prService(_baseBranch: string, _upstreamName: string) {
		return undefined;
	}

	checksMonitor(_sourceBranch: string) {
		return undefined;
	}
}
