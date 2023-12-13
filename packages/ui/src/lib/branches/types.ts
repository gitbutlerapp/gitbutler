import type { PullRequest } from '$lib/github/types';
import type { Author, Branch, RemoteBranch } from '$lib/vbranches/types';
import type iconsJson from '$lib/icons/icons.json';
import type { IconColor } from '$lib/icons/Icon.svelte';

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
	get displayName(): string {
		if (this.vbranch) return this.vbranch.name;
		if (this.pr) return this.pr.title;
		if (this.remoteBranch) return this.remoteBranch.displayName;
		return 'unknown';
	}

	get authors(): Author[] {
		const authors: Author[] = [];
		if (this.pr?.author) {
			authors.push(this.pr.author);
		}
		if (this.remoteBranch) {
			// TODO: Is there a better way to filter out duplicates?
			authors.push(
				...this.remoteBranch.authors.filter((a) => !authors.some((b) => a.email == b.email))
			);
		}
		if (this.vbranch) {
			authors.push({ name: 'you', email: 'none', isBot: false });
		}
		return authors;
	}

	get author(): Author | undefined {
		if (this.authors.length == 0) {
			return undefined;
		}
		return this.authors[0];
	}

	get icon(): keyof typeof iconsJson | undefined {
		if (this.vbranch) return 'branch';
		if (this.pr) return 'pr';
		if (this.remoteBranch) return 'branch';
		return undefined; // or implement a default icon?
	}

	get color(): IconColor {
		if (this.pr?.mergedAt) return 'pop';
		if (this.vbranch && this.vbranch.active == false) return 'warn';
		if (this.remoteBranch?.isMergeable) return 'success';
		return 'pop';
	}

	get modifiedAt(): Date | undefined {
		if (this.pr) return this.pr.modifiedAt || this.pr.createdAt;
		if (this.remoteBranch) return this.remoteBranch.lastCommitTs;
		return this.vbranch?.updatedTimestamp;
	}
}
