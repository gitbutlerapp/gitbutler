import type { PullRequest } from '$lib/gitHost/interface/types';
import type { Author, VirtualBranch, Branch } from '$lib/vbranches/types';

export class CombinedBranch {
	pr?: PullRequest;
	remoteBranch?: Branch;
	localBranch?: Branch;
	vbranch?: VirtualBranch;

	constructor({
		vbranch,
		remoteBranch,
		localBranch,
		pr
	}: {
		vbranch?: VirtualBranch;
		remoteBranch?: Branch;
		localBranch?: Branch;
		pr?: PullRequest;
	}) {
		this.vbranch = vbranch;
		this.remoteBranch = remoteBranch;
		this.localBranch = localBranch;
		this.pr = pr;
	}

	get upstreamSha(): string {
		return (
			this.pr?.sha ||
			this.remoteBranch?.sha ||
			this.localBranch?.sha ||
			this.vbranch?.upstream?.sha ||
			this.vbranch?.head ||
			'unknown'
		);
	}

	get displayName(): string {
		return (
			this.pr?.sourceBranch ||
			this.remoteBranch?.displayName ||
			this.localBranch?.displayName ||
			this.vbranch?.name ||
			'unknown'
		);
	}

	get authors(): Author[] {
		const authors: Author[] = [];
		if (this.pr?.author) {
			authors.push(this.pr.author);
		}
		if (this.branch) {
			if (this.branch.lastCommitAuthor) {
				authors.push({ name: this.branch.lastCommitAuthor });
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

	get icon():
		| 'remote-branch'
		| 'local-branch'
		| 'virtual-branch'
		| 'pr'
		| 'pr-draft'
		| 'pr-closed'
		| undefined {
		return this.currentState();
	}

	// GH colors reference https://github.blog/changelog/2021-06-08-new-issue-and-pull-request-state-icons
	get color(): 'neutral' | 'success' | 'purple' | undefined {
		if (this.pr?.mergedAt) return 'purple'; // merged PR
		if (this.pr) return 'success'; // open PR
		// if (this.remoteBranch?.isMergeable) return 'success'; // remote branches
		return 'neutral';
	}

	get modifiedAt(): Date | undefined {
		if (this.vbranch) return this.vbranch.updatedAt;
		if (this.branch) {
			return this.branch.lastCommitTimestampMs
				? new Date(this.branch.lastCommitTimestampMs)
				: undefined;
		}
		if (this.pr) {
			return this.pr.modifiedAt ? new Date(this.pr.modifiedAt) : undefined;
		}
	}

	get tooltip(): string | undefined {
		const currentState = this.currentState();
		switch (currentState) {
			case BranchState.VirtualBranch:
				return 'Virtual branch';
			case BranchState.RemoteBranch:
				return 'Remote branch';
			case BranchState.LocalBranch:
				return 'Local branch';
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
		if (this.branch) {
			identifiers.push(this.branch.displayName);
			this.branch.lastCommitAuthor && identifiers.push(this.branch.lastCommitAuthor);
		}

		return identifiers.map((identifier) => identifier.toLowerCase());
	}

	currentState(): BranchState | undefined {
		if (this.pr) return BranchState.PR;
		if (this.remoteBranch) return BranchState.RemoteBranch;
		if (this.localBranch) return BranchState.LocalBranch;
		if (this.vbranch) return BranchState.VirtualBranch;
		return undefined;
	}

	get branch() {
		// Prefer the local branch over the remote branch
		// We should always have at least one branch
		return this.localBranch || this.remoteBranch;
	}
}

enum BranchState {
	RemoteBranch = 'remote-branch',
	LocalBranch = 'local-branch',
	VirtualBranch = 'virtual-branch',
	PR = 'pr',
	PRDraft = 'pr-draft',
	PRClosed = 'pr-closed'
}
