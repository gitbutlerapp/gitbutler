import type { PullRequest } from '$lib/github/types';
import type { Author, RemoteBranch } from '$lib/vbranches/types';
import type iconsJson from '$lib/icons/icons.json';
import type { IconColor } from '$lib/icons/Icon.svelte';

export class CombinedBranch {
	pr?: PullRequest;
	branch?: RemoteBranch;

	constructor({ pr, remoteBranch }: { pr?: PullRequest; remoteBranch?: RemoteBranch }) {
		this.pr = pr;
		this.branch = remoteBranch;
	}

	get displayName(): string {
		if (this.pr) return this.pr.title;
		if (this.branch) return this.branch.displayName;
		return 'unknown';
	}

	get authors(): Author[] {
		if (this.pr?.author) {
			return [this.pr.author];
		} else if (this.branch) {
			return this.branch.authors;
		}
		throw 'No author found';
	}

	get author(): Author {
		if (this.pr?.author) return this.pr.author;
		else if (this.branch?.authors) return this.branch.authors[0];
		throw 'No author found';
	}

	get icon(): keyof typeof iconsJson {
		if (this.pr) return 'pr-16';
		else if (this.branch) return 'branch-16';
		throw 'No author found';
	}

	get color(): IconColor {
		if (this.pr?.mergedAt) return 'pop';
		else if (this.branch?.isMergeable) return 'success';
		return 'error';
	}

	get createdAt(): Date {
		if (this.pr) return this.pr.createdAt;
		if (this.branch) return this.branch.lastCommitTs;
		throw 'unknown error';
	}
	get modifiedAt(): Date {
		if (this.pr) return this.pr.modifiedAt || this.pr.createdAt;
		if (this.branch) return this.branch.firstCommitAt || this.branch.lastCommitTs;
		throw 'unknown error';
	}
}
