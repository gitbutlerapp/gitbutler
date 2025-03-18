import type { Forge, ForgeName } from '../interface/forge';
import type { ForgeBranch } from '../interface/forgeBranch';
import type { ForgeChecksMonitor } from '../interface/forgeChecksMonitor';
import type { ForgeIssueService } from '../interface/forgeIssueService';
import type { ForgeListingService } from '../interface/forgeListingService';
import type { ForgePrService } from '../interface/forgePrService';
import type { ForgeRepoService } from '../interface/forgeRepoService';

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
	checksMonitor(_branchName: string): ForgeChecksMonitor | undefined {
		return undefined;
	}
	branch(_name: string): ForgeBranch | undefined {
		return undefined;
	}
	commitUrl(_id: string): string | undefined {
		return undefined;
	}
}
