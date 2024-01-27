import type { PullRequest } from '$lib/github/types';
import type { Branch, RemoteBranch } from '$lib/vbranches/types';
import { CombinedBranch } from '$lib/branches/types';
import { Observable, combineLatest } from 'rxjs';
import { startWith, switchMap } from 'rxjs/operators';
import type { RemoteBranchService } from '$lib/stores/remoteBranches';
import type { GitHubService } from '$lib/github/service';
import type { VirtualBranchService } from '$lib/vbranches/branchStoresCache';
import type { Transaction } from '@sentry/sveltekit';
import { capture } from '$lib/analytics/posthog';

export class BranchService {
	public branches$: Observable<CombinedBranch[]>;

	constructor(
		private vbranchService: VirtualBranchService,
		remoteBranchService: RemoteBranchService,
		private githubService: GitHubService
	) {
		const vbranchesWithEmpty$ = vbranchService.branches$.pipe(startWith([]));
		const branchesWithEmpty$ = remoteBranchService.branches$.pipe(startWith([]));
		const prWithEmpty$ = githubService.prs$.pipe(startWith([]));

		this.branches$ = combineLatest([vbranchesWithEmpty$, branchesWithEmpty$, prWithEmpty$]).pipe(
			switchMap(
				([vbranches, remoteBranches, pullRequests]) =>
					new Observable<CombinedBranch[]>((observer) => {
						const contributions = mergeBranchesAndPrs(
							vbranches,
							pullRequests,
							remoteBranches || []
						);
						observer.next(contributions);
					})
			)
		);
	}

	async createPr(
		branch: Branch,
		baseBranch: string,
		sentryTxn: Transaction
	): Promise<PullRequest | undefined> {
		// Using this mutable variable while investigating why branch variable
		// does not seem to update reliably.
		// TODO: This needs to be fixed and removed.
		let newBranch: Branch | undefined;

		// Push if local commits
		if (branch.commits.some((c) => !c.isRemote)) {
			const pushBranchSpan = sentryTxn.startChild({ op: 'branch_push' });
			newBranch = await this.vbranchService.pushBranch(branch.id, branch.requiresForce);
			pushBranchSpan.finish();
		} else {
			newBranch = branch;
		}

		if (!newBranch) {
			const err = 'branch push failed';
			capture(err, { upstream: branch.upstreamName });
			throw err;
		}

		if (!newBranch.upstreamName) {
			throw 'Cannot create PR without remote branch name';
		}

		const createPrSpan = sentryTxn.startChild({ op: 'pr_api_create' });

		try {
			return await this.githubService.createPullRequest(
				baseBranch,
				newBranch.name,
				newBranch.notes,
				newBranch.id,
				newBranch.upstreamName
			);
		} finally {
			createPrSpan.finish();
		}
	}
}

function mergeBranchesAndPrs(
	vbranches: Branch[] | undefined,
	pullRequests: PullRequest[],
	remoteBranches: RemoteBranch[]
): CombinedBranch[] {
	const contributions: CombinedBranch[] = [];

	// First we add everything with a virtual branch
	if (vbranches) {
		contributions.push(
			...vbranches.map((vb) => {
				const upstream = vb.upstream?.upstream;
				const pr = upstream
					? pullRequests.find((pr) => isBranchNameMatch(pr.targetBranch, upstream))
					: undefined;
				return new CombinedBranch({ vbranch: vb, remoteBranch: vb.upstream, pr });
			})
		);
	}

	// Then remote branches that have no virtual branch, combined with pull requests if present
	contributions.push(
		...remoteBranches
			.filter((rb) => !vbranches?.some((vb) => isBranchNameMatch(rb.name, vb.upstreamName)))
			.map((rb) => {
				const pr = pullRequests.find((pr) => isBranchNameMatch(pr.targetBranch, rb.name));
				return new CombinedBranch({ remoteBranch: rb, pr });
			})
	);

	// And finally pull requests that lack any corresponding branch
	contributions.push(
		...pullRequests
			.filter((pr) => !remoteBranches.some((rb) => isBranchNameMatch(pr.targetBranch, rb.name)))
			.map((pr) => {
				return new CombinedBranch({ pr });
			})
	);

	// This should be everything considered a branch in one list
	const filtered = contributions
		.filter((b) => !b.vbranch || !b.vbranch.active)
		.sort((a, b) => {
			return (a.modifiedAt || new Date(0)) < (b.modifiedAt || new Date(0)) ? 1 : -1;
		});
	return filtered;
}

function isBranchNameMatch(left: string | undefined, right: string | undefined): boolean {
	if (!left || !right) return false;
	return left.split('/').pop() === right.split('/').pop();
}
