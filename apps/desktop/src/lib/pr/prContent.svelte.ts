import type { DetailedCommit } from '$lib/vbranches/types';

export class ReactivePRTitle {
	value = $state<string>('');

	constructor(
		private isDisplay: boolean,
		private existingTitle: string | undefined,
		private commits: DetailedCommit[],
		private branchName: string
	) {
		this.value = this.getDefaultTitle();
	}

	private getDefaultTitle(): string {
		if (this.isDisplay) return this.existingTitle ?? '';
		// In case of a single commit, use the commit summary for the title
		if (this.commits.length === 1) {
			const commit = this.commits[0];
			return commit?.descriptionTitle ?? '';
		}
		return this.branchName;
	}

	set(value: string) {
		this.value = value;
	}
}

export class ReactivePRBody {
	value = $state<string>('');

	constructor(
		private isDisplay: boolean,
		private branchDescription: string | undefined,
		private existingBody: string | undefined,
		private commits: DetailedCommit[],
		private templateBody: string | undefined
	) {
		this.value = this.getDefaultBody();
	}

	getDefaultBody(): string {
		if (this.isDisplay) return this.existingBody ?? '';
		if (this.branchDescription) return this.branchDescription;
		if (this.templateBody) return this.templateBody;
		// In case of a single commit, use the commit description for the body
		if (this.commits.length === 1) {
			const commit = this.commits[0];
			return commit?.descriptionBody ?? '';
		}
		return '';
	}

	set(value: string) {
		this.value = value;
	}

	append(value: string) {
		this.value += value;
	}

	reset() {
		this.value = '';
	}
}
