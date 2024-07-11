import { capture } from '$lib/analytics/posthog';
import { CombinedBranch } from '$lib/branches/types';
import { Observable, combineLatest, of } from 'rxjs';
import { catchError, shareReplay, startWith, switchMap } from 'rxjs/operators';
import type { GitHubService } from '$lib/github/service';
import type { PullRequest } from '$lib/github/types';
import type { RemoteBranchService } from '$lib/stores/remoteBranches';
import type { BranchController } from '$lib/vbranches/branchController';
import type { Branch, RemoteBranch } from '$lib/vbranches/types';
import type { VirtualBranchService } from '$lib/vbranches/virtualBranch';

export class BranchService {
	readonly branches$: Observable<CombinedBranch[]>;

	constructor(
		virtualBranchService: VirtualBranchService,
		remoteBranchService: RemoteBranchService,
		private githubService: GitHubService,
		private branchController: BranchController
	) {
		const virtualBranchesWithEmpty$ = virtualBranchService.activeBranches$.pipe(
			startWith([]),
			catchError(() => of(undefined))
		);
		const branchesWithEmpty$ = remoteBranchService.branches$.pipe(
			startWith([]),
			catchError(() => of(undefined))
		);
		const prWithEmpty$ = githubService.prs$.pipe(catchError(() => of(undefined)));

		this.branches$ = combineLatest([
			virtualBranchesWithEmpty$,
			branchesWithEmpty$,
			prWithEmpty$
		]).pipe(
			switchMap(([virtualBranches, remoteBranches, pullRequests]) => {
				return new Observable<CombinedBranch[]>((observer) => {
					const contributions = mergeBranchesAndPrs(
						virtualBranches,
						pullRequests,
						remoteBranches || []
					);
					observer.next(contributions);
					observer.complete();
				});
			}),
			shareReplay(1)
		);
	}

	async createPr(
		branch: Branch,
		baseBranch: string,
		draft: boolean
	): Promise<PullRequest | undefined> {
		// Using this mutable variable while investigating why branch variable
		// does not seem to update reliably.
		// TODO: This needs to be fixed and removed.
		let newBranch: Branch | undefined;

		// Push if local commits
		if (branch.commits.some((c) => !c.isRemote)) {
			newBranch = await this.branchController.pushBranch(branch.id, branch.requiresForce);
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

		let title = newBranch.name;
		let body = newBranch.notes;

		// In case of a single commit, use the commit summary and description for the title and
		// description of the PR
		if (newBranch.commits.length === 1) {
			const commit = newBranch.commits[0];
			if (commit.descriptionTitle) title = commit.descriptionTitle;
			if (commit.descriptionBody) body = commit.descriptionBody;
		}

		const resp = await this.githubService.createPullRequest(
			baseBranch,
			title,
			body,
			newBranch.id,
			newBranch.upstreamName,
			draft
		);
		if ('pr' in resp) return resp.pr;
		else throw resp.err;
	}
}

function mergeBranchesAndPrs(
	virtualBranches: Branch[] | undefined,
	pullRequests: PullRequest[] | undefined,
	remoteBranches: RemoteBranch[] | undefined
): CombinedBranch[] {
	const contributions: CombinedBranch[] = [];

	// Then remote branches that have no virtual branch, combined with pull requests if present
	if (remoteBranches) {
		contributions.push(
			...remoteBranches.map((rb) => {
				const pr = pullRequests?.find((pr) => pr.sourceBranch === rb.givenName);
				return new CombinedBranch({ remoteBranch: rb, pr });
			})
		);
	}

	// And finally pull requests that lack any corresponding branch
	if (pullRequests) {
		contributions.push(
			...pullRequests
				.filter((pr) => !contributions.some((cb) => pr.sourceBranch === cb.givenName))
				.map((pr) => {
					return new CombinedBranch({ pr });
				})
		);
	}

	const virtualBranchNames =
		virtualBranches?.map(
			(virtualBranch) => virtualBranch.upstream?.givenName ?? virtualBranch.name
		) || [];

	// This should be everything considered a branch in one list
	const filtered = contributions
		.filter((combinedBranch) => !virtualBranchNames.includes(combinedBranch.givenName))
		.sort((a, b) => {
			return (a.modifiedAt || new Date(0)) < (b.modifiedAt || new Date(0)) ? 1 : -1;
		});
	return filtered;
}
