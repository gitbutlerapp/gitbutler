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
	public branches$: Observable<CombinedBranch[]>;

	constructor(
		private vbranchService: VirtualBranchService,
		remoteBranchService: RemoteBranchService,
		private githubService: GitHubService,
		private branchController: BranchController
	) {
		const vbranchesWithEmpty$ = vbranchService.branches$.pipe(
			startWith([]),
			catchError(() => of(undefined))
		);
		const branchesWithEmpty$ = remoteBranchService.branches$.pipe(
			startWith([]),
			catchError(() => of(undefined))
		);
		const prWithEmpty$ = githubService.prs$.pipe(catchError(() => of(undefined)));

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

	async reloadVirtualBranches() {
		await this.vbranchService.reload();
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
				const pr = pullRequests?.find((pr) => pr.sha === vb.head);
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
		.filter((b) => !b.vbranch || !b.vbranch.active)
		.sort((a, b) => {
			return (a.modifiedAt || new Date(0)) < (b.modifiedAt || new Date(0)) ? 1 : -1;
		});
	return filtered;
}
