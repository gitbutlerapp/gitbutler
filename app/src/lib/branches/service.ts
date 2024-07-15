import { GivenNameBranchGrouping } from '$lib/branches/types';
import { buildContextStore } from '$lib/utils/context';
import { groupBy } from '$lib/utils/groupBy';
import { derived, readable, writable, type Readable } from 'svelte/store';
import type { GitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
import type { PullRequest } from '$lib/gitHost/interface/types';
import type { RemoteBranchService } from '$lib/stores/remoteBranches';
import type { VirtualBranch, Branch } from '$lib/vbranches/types';
import type { VirtualBranchService } from '$lib/vbranches/virtualBranch';

export const [getBranchServiceStore, createBranchServiceStore] = buildContextStore<
	BranchService | undefined
>('branchService');

export class BranchService {
	readonly branches: Readable<GivenNameBranchGrouping[]>;
	readonly error = writable();

	constructor(
		vbranchService: VirtualBranchService,
		remoteBranchService: RemoteBranchService,
		gitPrService: GitHostListingService | undefined
	) {
		const vbranches = vbranchService.branches;
		const branches = remoteBranchService.branches;
		const prs = gitPrService ? gitPrService.prs : readable([]);

		this.branches = derived(
			[vbranches, branches, prs],
			([vbranches, remoteBranches, pullRequests]) => {
				return mergeBranchesAndPrs(vbranches, pullRequests, remoteBranches || []);
			}
		);
	}
}

function mergeBranchesAndPrs(
	virtualBranches: VirtualBranch[] = [],
	pullRequests: PullRequest[] = [],
	remoteBranches: Branch[] = []
): GivenNameBranchGrouping[] {
	const groupedPullRequests = groupBy(pullRequests, (pullRequest) => pullRequest.sourceBranch);
	const groupedRemoteBranches = groupBy(remoteBranches, (remoteBranch) => remoteBranch.givenName);

	const branchNames = new Set<string>([
		...Object.keys(groupedPullRequests),
		...Object.keys(groupedRemoteBranches)
	]);

	const virtualBranchNames = virtualBranches?.map(
		(virtualBranch) => virtualBranch.upstream?.givenName ?? virtualBranch.name
	);

	// We don't want to list virtual branches so remove their names from the pool of all names
	virtualBranchNames.forEach((virtualBranchName) => branchNames.delete(virtualBranchName));

	const combinedBranches = [...branchNames].map((branchName) => {
		const pullRequests = groupedPullRequests[branchName] || [];
		const remoteBranches = groupedRemoteBranches[branchName] || [];

		return new GivenNameBranchGrouping({
			pullRequests: pullRequests,
			branches: remoteBranches
		});
	});

	return combinedBranches;
}
