import type { PullRequest } from '$lib/github/types';
import type { Branch, RemoteBranch } from '$lib/vbranches/types';
import { CombinedBranch } from '$lib/branches/types';
import { Observable, combineLatest } from 'rxjs';
import { startWith, switchMap } from 'rxjs/operators';
import type { RemoteBranchService } from '$lib/stores/remoteBranches';
import type { PrService } from '$lib/github/pullrequest';
import type { VirtualBranchService } from '$lib/vbranches/branchStoresCache';

export class BranchService {
	public branches$: Observable<CombinedBranch[]>;

	constructor(
		vbranchService: VirtualBranchService,
		remoteBranchService: RemoteBranchService,
		prService: PrService
	) {
		const vbranchesWithEmpty$ = vbranchService.branches$.pipe(startWith([]));
		const branchesWithEmpty$ = remoteBranchService.branches$.pipe(startWith([]));
		const prWithEmpty$ = prService.prs$.pipe(startWith([]));

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
}

function mergeBranchesAndPrs(
	vbranches: Branch[],
	pullRequests: PullRequest[],
	remoteBranches: RemoteBranch[]
): CombinedBranch[] {
	const contributions: CombinedBranch[] = [];

	// First we add everything with a virtual branch
	contributions.push(
		...vbranches.map((vb) => {
			const upstream = vb.upstream?.upstream;
			const pr = upstream
				? pullRequests.find((pr) => isBranchNameMatch(pr.targetBranch, upstream))
				: undefined;
			return new CombinedBranch({ vbranch: vb, remoteBranch: vb.upstream, pr });
		})
	);

	// Then remote branches that have no virtual branch, combined with pull requests if present
	contributions.push(
		...remoteBranches
			.filter((rb) => !vbranches.some((vb) => isBranchNameMatch(rb.name, vb.upstreamName)))
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
