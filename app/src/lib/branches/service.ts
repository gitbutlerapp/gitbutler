import { CombinedBranch } from '$lib/branches/types';
import { buildContextStore } from '$lib/utils/context';
import { derived, readable, writable, type Readable } from 'svelte/store';
import type { GitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
import type { PullRequest } from '$lib/gitHost/interface/types';
import type { RemoteBranchService } from '$lib/stores/remoteBranches';
import type { VirtualBranch, RemoteBranch } from '$lib/vbranches/types';
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
	vbranches: VirtualBranch[] | undefined,
	pullRequests: PullRequest[] | undefined,
	remoteBranches: RemoteBranch[] | undefined
): CombinedBranch[] {
	const contributions: CombinedBranch[] = [];

	// First we add everything with a virtual branch
	if (vbranches) {
		contributions.push(
			...vbranches.map((vb) => {
				const pr = pullRequests?.find((pr) => pr.sourceBranch === vb.upstreamName);
				return new CombinedBranch({ vbranch: vb, remoteBranch: vb.upstream, pr });
			})
		);
	}

	// Then remote branches that have no virtual branch, combined with pull requests if present
	if (remoteBranches) {
		contributions.push(
			...remoteBranches
				.filter((rb) => !contributions.some((cb) => rb.sha === cb.upstreamSha))
				.map((rb) => {
					const pr = pullRequests?.find((pr) => pr.sha === rb.sha);
					return new CombinedBranch({ remoteBranch: rb, pr });
				})
		);
	}

	// And finally pull requests that lack any corresponding branch
	if (pullRequests) {
		contributions.push(
			...pullRequests
				.filter((pr) => !contributions.some((cb) => pr.sha === cb.upstreamSha))
				.map((pr) => {
					return new CombinedBranch({ pr });
				})
		);
	}

	// This should be everything considered a branch in one list
	const filtered = contributions
		.filter((b) => !b.vbranch)
		.sort((a, b) => {
			return (a.modifiedAt || new Date(0)) < (b.modifiedAt || new Date(0)) ? 1 : -1;
		});
	return filtered;
}
