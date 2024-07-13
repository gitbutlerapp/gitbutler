import { CombinedBranch } from '$lib/branches/types';
import { observableToStore, storeToObservable } from '$lib/rxjs/store';
import { buildContextStore } from '$lib/utils/context';
import { Observable, combineLatest, of } from 'rxjs';
import { catchError, shareReplay, startWith, switchMap } from 'rxjs/operators';
import type { HostedGitListingService } from '$lib/gitHost/interface/hostedGitListingService';
import type { PullRequest } from '$lib/gitHost/interface/types';
import type { RemoteBranchService } from '$lib/stores/remoteBranches';
import type { Branch, RemoteBranch } from '$lib/vbranches/types';
import type { VirtualBranchService } from '$lib/vbranches/virtualBranch';
import type { Readable } from 'svelte/store';

export const [getBranchServiceStore, createBranchServiceStore] = buildContextStore<
	BranchService | undefined
>('branchService');

export class BranchService {
	readonly branches$: Observable<CombinedBranch[]>;
	readonly branches: Readable<CombinedBranch[]>;
	readonly error: Readable<any>;

	constructor(
		vbranchService: VirtualBranchService,
		remoteBranchService: RemoteBranchService,
		hostedGitService: HostedGitListingService | undefined
	) {
		const vbranchesWithEmpty$ = vbranchService.branches$.pipe(
			startWith([]),
			catchError(() => of(undefined))
		);
		const branchesWithEmpty$ = remoteBranchService.branches$.pipe(
			startWith([]),
			catchError(() => of(undefined))
		);
		const prWithEmpty$ = hostedGitService ? storeToObservable(hostedGitService.prs) : of([]);

		this.branches$ = combineLatest([vbranchesWithEmpty$, branchesWithEmpty$, prWithEmpty$]).pipe(
			switchMap(([vbranches, remoteBranches, pullRequests]) => {
				return new Observable<CombinedBranch[]>((observer) => {
					const contributions = mergeBranchesAndPrs(vbranches, pullRequests, remoteBranches || []);
					observer.next(contributions);
					observer.complete();
				});
			}),
			shareReplay(1)
		);
		[this.branches, this.error] = observableToStore(this.branches$);
	}
}

function mergeBranchesAndPrs(
	vbranches: Branch[] | undefined,
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
