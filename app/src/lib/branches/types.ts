import type { PullRequest } from '$lib/github/types';
import type { Author, RemoteBranch } from '$lib/vbranches/types';

export class CombinedBranch {
	primaryPullRequest?: PullRequest;
	primaryRemoteBranch?: RemoteBranch;

	otherPullRequests: PullRequest[];
	otherRemoteBranches: RemoteBranch[];

	constructor({
		primaryRemoteBranch: remoteBranch,
		otherRemoteBranches,
		primaryPullRequest: pr,
		otherPullRequests
	}: {
		primaryRemoteBranch?: RemoteBranch;
		primaryPullRequest?: PullRequest;
		otherRemoteBranches: RemoteBranch[];
		otherPullRequests: PullRequest[];
	}) {
		this.primaryRemoteBranch = remoteBranch;
		this.primaryPullRequest = pr;
		this.otherPullRequests = otherPullRequests;
		this.otherRemoteBranches = otherRemoteBranches;
	}

	get upstreamSha(): string {
		return this.primaryPullRequest?.sha || this.primaryRemoteBranch?.sha || 'unknown';
	}

	get displayName(): string {
		console.log(this.primaryPullRequest);
		console.log(this.primaryRemoteBranch);
		return (
			this.primaryPullRequest?.sourceBranch || this.primaryRemoteBranch?.displayName || 'unknown'
		);
	}

	get givenName(): string {
		return (
			this.primaryPullRequest?.sourceBranch || this.primaryRemoteBranch?.givenName || 'unknown'
		);
	}

	get authors(): Author[] {
		const authors: Author[] = [];
		if (this.primaryPullRequest?.author) {
			authors.push(this.primaryPullRequest.author);
		}
		if (this.primaryRemoteBranch) {
			if (this.primaryRemoteBranch.lastCommitAuthor) {
				authors.push({ name: this.primaryRemoteBranch.lastCommitAuthor });
			}
		}
		return authors;
	}

	get author(): Author | undefined {
		if (this.authors.length === 0) {
			return undefined;
		}
		return this.authors[0];
	}

	get icon(): 'remote-branch' | 'virtual-branch' | 'pr' | 'pr-draft' | 'pr-closed' | undefined {
		return this.currentState();
	}

	// GH colors reference https://github.blog/changelog/2021-06-08-new-issue-and-pull-request-state-icons
	get color(): 'neutral' | 'success' | 'purple' | undefined {
		if (this.primaryPullRequest?.mergedAt) return 'purple'; // merged PR
		if (this.primaryPullRequest) return 'success'; // open PR
		// if (this.remoteBranch?.isMergeable) return 'success'; // remote branches
		return 'neutral';
	}

	get modifiedAt(): Date | undefined {
		if (this.primaryRemoteBranch) {
			return this.primaryRemoteBranch.lastCommitTimestampMs
				? new Date(this.primaryRemoteBranch.lastCommitTimestampMs)
				: undefined;
		}
	}

	get tooltip(): string | undefined {
		const currentState = this.currentState();
		switch (currentState) {
			case BranchState.VirtualBranch:
				return 'Virtual branch';
			case BranchState.RemoteBranch:
				return 'Remote branch';
			case BranchState.PR:
				return 'Pull Request';
			case BranchState.PRClosed:
				return 'Closed Pull Request';
			case BranchState.PRDraft:
				return 'Draft Pull Request';
		}
	}

	get searchableIdentifiers() {
		const identifiers = [];

		if (this.primaryPullRequest) {
			identifiers.push(this.primaryPullRequest.title);
			identifiers.push(this.primaryPullRequest.sourceBranch);
			this.primaryPullRequest.author?.email &&
				identifiers.push(this.primaryPullRequest.author.email);
			this.primaryPullRequest.author?.name && identifiers.push(this.primaryPullRequest.author.name);
		}
		if (this.primaryRemoteBranch) {
			identifiers.push(this.primaryRemoteBranch.displayName);
			this.primaryRemoteBranch.lastCommitAuthor &&
				identifiers.push(this.primaryRemoteBranch.lastCommitAuthor);
		}

		return identifiers.map((identifier) => identifier.toLowerCase());
	}

	currentState(): BranchState | undefined {
		if (this.primaryPullRequest) return BranchState.PR;
		if (this.primaryRemoteBranch) return BranchState.RemoteBranch;
		return undefined;
	}
}

enum BranchState {
	RemoteBranch = 'remote-branch',
	VirtualBranch = 'virtual-branch',
	PR = 'pr',
	PRDraft = 'pr-draft',
	PRClosed = 'pr-closed'
}
