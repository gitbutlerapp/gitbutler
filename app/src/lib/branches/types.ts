import type { PullRequest } from '$lib/github/types';
import type { Author, Branch, RemoteBranch } from '$lib/vbranches/types';

export class CombinedBranch {
	pr?: PullRequest;
	remoteBranch?: RemoteBranch;
	vbranch?: Branch;

	constructor({
		vbranch,
		remoteBranch,
		pr
	}: {
		vbranch?: Branch;
		remoteBranch?: RemoteBranch;
		pr?: PullRequest;
	}) {
		this.vbranch = vbranch;
		this.remoteBranch = remoteBranch;
		this.pr = pr;
	}

	get upstreamSha(): string {
		return (
			this.pr?.sha ||
			this.remoteBranch?.sha ||
			this.vbranch?.upstream?.sha ||
			this.vbranch?.head ||
			'unknown'
		);
	}

	get displayName(): string {
		return (
			this.pr?.sourceBranch || this.remoteBranch?.displayName || this.vbranch?.name || 'unknown'
		);
	}

	get authors(): Author[] {
		const authors: Author[] = [];
		if (this.pr?.author) {
			authors.push(this.pr.author);
		}
		if (this.remoteBranch) {
			if (this.remoteBranch.lastCommitAuthor) {
				authors.push({ name: this.remoteBranch.lastCommitAuthor });
			}
		}
		if (this.vbranch) {
			authors.push({ name: 'you', email: 'none', isBot: false });
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
	get color(): 'neutral' | 'success' | 'pop' | 'purple' | undefined {
		if (this.pr?.mergedAt) return 'purple'; // merged PR
		if (this.pr) return 'success'; // open PR
		if (this.vbranch && this.vbranch.active === false) return 'pop'; // stashed virtual branches
		// if (this.remoteBranch?.isMergeable) return 'success'; // remote branches
		return 'neutral';
	}

	get modifiedAt(): Date | undefined {
		if (this.vbranch) return this.vbranch.updatedAt;
		if (this.remoteBranch) {
			return this.remoteBranch.lastCommitTimestampMs
				? new Date(this.remoteBranch.lastCommitTimestampMs)
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

		if (this.vbranch) identifiers.push(this.vbranch.name);
		if (this.pr) {
			identifiers.push(this.pr.title);
			identifiers.push(this.pr.sourceBranch);
			this.pr.author?.email && identifiers.push(this.pr.author.email);
			this.pr.author?.name && identifiers.push(this.pr.author.name);
		}
		if (this.remoteBranch) {
			identifiers.push(this.remoteBranch.displayName);
			this.remoteBranch.lastCommitAuthor && identifiers.push(this.remoteBranch.lastCommitAuthor);
		}

		return identifiers.map((identifier) => identifier.toLowerCase());
	}

	currentState(): BranchState | undefined {
		if (this.pr) return BranchState.PR;
		if (this.remoteBranch) return BranchState.RemoteBranch;
		if (this.vbranch) return BranchState.VirtualBranch;
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
