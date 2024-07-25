import { CombinedBranch } from '$lib/branches/types';
import { buildContextStore } from '$lib/utils/context';
import { groupBy } from '$lib/utils/groupBy';
import { derived, readable, writable, type Readable } from 'svelte/store';
import type { NameNormalizationService } from '$lib/branches/nameNormalizationService';
import type { GitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
import type { PullRequest } from '$lib/gitHost/interface/types';
import type { RemoteBranchService } from '$lib/stores/remoteBranches';
import type { VirtualBranch, Branch } from '$lib/vbranches/types';
import type { VirtualBranchService } from '$lib/vbranches/virtualBranch';

export const [getBranchServiceStore, createBranchServiceStore] = buildContextStore<
	BranchService | undefined
>('branchService');

export class BranchService {
	readonly branches: Readable<CombinedBranch[]>;
	readonly error = writable();

	constructor(
		vbranchService: VirtualBranchService,
		remoteBranchService: RemoteBranchService,
		gitPrService: GitHostListingService | undefined,
		nameNormalizationService: NameNormalizationService
	) {
		const vbranches = vbranchService.branches;
		const branches = remoteBranchService.branches;
		const prs = gitPrService ? gitPrService.prs : readable([]);

		this.branches = derived(
			[vbranches, branches, prs],
			([vbranches, remoteBranches, pullRequests], set) => {
				// derived with a set does not allow you to return a promise
				mergeBranchesAndPrs(
					vbranches || [],
					pullRequests || [],
					remoteBranches || [],
					nameNormalizationService
				).then((combinedBranches) => {
					set(combinedBranches);
				});
			},
			[] as CombinedBranch[] // Use an empty array as the default, with sufficient typing
		);
	}
}

async function mergeBranchesAndPrs(
	virtualBranches: VirtualBranch[],
	pullRequests: PullRequest[],
	branches: Branch[],
	nameNormalizationService: NameNormalizationService
): Promise<CombinedBranch[]> {
	const contributions: CombinedBranch[] = [];

	const groupedBranches = groupBy(branches, (branch) => branch.givenName);

	for (const [_, branches] of Object.entries(groupedBranches)) {
		// There should only ever be one local reference for a particular given name
		const localBranch = branches.find((branch) => !branch.isRemote);
		const remoteBranches = branches.filter((branch) => branch.isRemote);

		// There must be a local branch if there are no remote branches
		if (remoteBranches.length === 0) {
			contributions.push(new CombinedBranch({ localBranch }));

			continue;
		}

		remoteBranches.forEach((remoteBranch) => {
			contributions.push(new CombinedBranch({ remoteBranch, localBranch }));
		});
	}

	contributions.forEach((contribution) => {
		const pullRequest = pullRequests.find(
			// This may be over-sensitive in rare cases, but is preferable to using the head sha
			(pullRequest) => contribution.remoteBranch?.givenName === pullRequest.sourceBranch
		);

		if (pullRequest) {
			contribution.pr = pullRequest;
		}
	});

	const normalizedVirtualBranchNames = new Set(
		await Promise.all(
			virtualBranches.map(
				async (virtualBranch) => await nameNormalizationService.normalize(virtualBranch.name)
			)
		)
	);

	// This should be everything considered a branch in one list
	const filtered = contributions
		.filter((combinedBranch) => !normalizedVirtualBranchNames.has(combinedBranch.branch!.givenName))
		.sort((a, b) => {
			return (a.modifiedAt || new Date(0)) < (b.modifiedAt || new Date(0)) ? 1 : -1;
		});
	return filtered;
}
