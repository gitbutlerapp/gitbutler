import type { PullRequest } from '$lib/gitHost/interface/types';
import type { Author, Branch } from '$lib/vbranches/types';

/**
 * A grouping of pull requests and branches based on their given name
 *
 * The given name of a branch is the name a user gave it, without the remote
 * name attached
 *
 * IE for refs/heads/my-branch, the given name is my-branch
 * for refs/remotes/origin/my-branch, the given name is my-branch
 */
export class GivenNameBranchGrouping {
	pullRequests: PullRequest[];
	branches: Branch[];

	constructor({ pullRequests, branches }: { branches: Branch[]; pullRequests: PullRequest[] }) {
		// If there are no pull requests or branches passed in, it is an absurd situation so we ought to panic
		if (pullRequests.length === 0 && branches.length === 0) {
			throw new Error('Combined branch created without any branches or pull requests attached');
		}

		this.pullRequests = pullRequests;
		this.branches = branches;
	}

	get primaryPullRequest(): PullRequest | undefined {
		return this.pullRequests[0];
	}

	get primaryBranch(): Branch | undefined {
		return this.branches[0];
	}

	get upstreamSha(): string {
		return this.primaryPullRequest?.sha || this.primaryBranch?.sha || 'unknown';
	}

	get displayName(): string {
		return this.primaryPullRequest?.sourceBranch || this.primaryBranch?.displayName || 'unknown';
	}

	get givenName(): string {
		return this.primaryPullRequest?.sourceBranch || this.primaryBranch?.givenName || 'unknown';
	}

	get authors(): Author[] {
		const authors: Author[] = [];
		if (this.primaryPullRequest?.author) {
			authors.push(this.primaryPullRequest.author);
		}
		if (this.primaryBranch) {
			if (this.primaryBranch.lastCommitAuthor) {
				authors.push({ name: this.primaryBranch.lastCommitAuthor });
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
		if (this.primaryBranch) {
			return this.primaryBranch.lastCommitTimestampMs
				? new Date(this.primaryBranch.lastCommitTimestampMs)
				: undefined;
		}
	}

	get tooltip(): string | undefined {
		const currentState = this.currentState();
		switch (currentState) {
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
		if (this.primaryBranch) {
			identifiers.push(this.primaryBranch.displayName);
			this.primaryBranch.lastCommitAuthor && identifiers.push(this.primaryBranch.lastCommitAuthor);
		}

		return identifiers.map((identifier) => identifier.toLowerCase());
	}

	currentState(): BranchState | undefined {
		if (this.primaryPullRequest) return BranchState.PR;
		if (this.primaryBranch) return BranchState.RemoteBranch;
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
