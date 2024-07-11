import { capture } from '$lib/analytics/posthog';
import { GivenNameBranchGrouping } from '$lib/branches/types';
import { groupBy } from '$lib/utils/groupBy';
import { Observable, combineLatest, of } from 'rxjs';
import { catchError, shareReplay, startWith, switchMap } from 'rxjs/operators';
import type { GitHubService } from '$lib/github/service';
import type { PullRequest } from '$lib/github/types';
import type { RemoteBranchService } from '$lib/stores/remoteBranches';
import type { BranchController } from '$lib/vbranches/branchController';
import type { VirtualBranch, Branch } from '$lib/vbranches/types';
import type { VirtualBranchService } from '$lib/vbranches/virtualBranch';

export class BranchService {
	readonly branches$: Observable<GivenNameBranchGrouping[]>;

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
				return new Observable<GivenNameBranchGrouping[]>((observer) => {
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
		branch: VirtualBranch,
		baseBranch: string,
		draft: boolean
	): Promise<PullRequest | undefined> {
		// Using this mutable variable while investigating why branch variable
		// does not seem to update reliably.
		// TODO: This needs to be fixed and removed.
		let newBranch: VirtualBranch | undefined;

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
