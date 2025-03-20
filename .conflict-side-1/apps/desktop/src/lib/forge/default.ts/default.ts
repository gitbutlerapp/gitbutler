import type { Forge, ForgeName } from '$lib/forge/interface/forge';
import type { ForgeBranch } from '$lib/forge/interface/forgeBranch';
import type { ChecksService } from '$lib/forge/interface/forgeChecksMonitor';
import type { ForgeIssueService } from '$lib/forge/interface/forgeIssueService';
import type { ForgeListingService } from '$lib/forge/interface/forgeListingService';
import type { ForgePrService } from '$lib/forge/interface/forgePrService';
import type { ForgeRepoService } from '$lib/forge/interface/forgeRepoService';

export class DefaultForge implements Forge {
	name: ForgeName;

	constructor() {
		this.name = 'default';
	}

	get listService(): ForgeListingService | undefined {
		return undefined;
	}
	get issueService(): ForgeIssueService | undefined {
		return undefined;
	}
	get prService(): ForgePrService | undefined {
		return undefined;
	}
	get repoService(): ForgeRepoService | undefined {
		return undefined;
	}
	get checks(): ChecksService | undefined {
		return undefined;
	}
	branch(_name: string): ForgeBranch | undefined {
		return undefined;
	}
	commitUrl(_id: string): string | undefined {
		return undefined;
	}
}
