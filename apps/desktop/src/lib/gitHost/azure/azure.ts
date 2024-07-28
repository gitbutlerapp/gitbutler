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
	webUrl: string;

	constructor(
		repo: RepoInfo,
		private baseBranch: string,
		private fork?: string
	) {
		this.webUrl = `https://${AZURE_DOMAIN}/${repo.owner}/${repo.name}`;
	}

	branch(name: string) {
		return new AzureBranch(name, this.baseBranch, this.webUrl, this.fork);
	}

	commitUrl(id: string): string {
		return `${this.webUrl}/commit/${id}`;
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
