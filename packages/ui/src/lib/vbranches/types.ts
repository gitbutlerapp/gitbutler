import 'reflect-metadata';
import { Type, Transform } from 'class-transformer';
import type { Readable, WritableLoadable } from '@square/svelte-store';
import type { LoadState, VisitedMap } from '@square/svelte-store/lib/async-stores/types';
import { max } from 'date-fns';

export class Hunk {
	id!: string;
	diff!: string;
	@Transform((obj) => {
		return new Date(obj.value);
	})
	modifiedAt!: Date;
	filePath!: string;
	locked!: boolean;

	selected!: boolean;
}

export class File {
	id!: string;
	path!: string;
	@Type(() => Hunk)
	hunks!: Hunk[];
	expanded?: boolean;
	@Transform((obj) => new Date(obj.value))
	modifiedAt!: Date;
	conflicted!: boolean;
	content!: string;
	binary!: boolean;

	getSummary() {
		const { added, removed } = this.hunks
			.map((h) => h.diff.split('\n'))
			.reduce(
				(acc, lines) => ({
					added: acc.added + lines.filter((l) => l.startsWith('+')).length,
					removed: acc.removed + lines.filter((l) => l.startsWith('-')).length
				}),
				{ added: 0, removed: 0 }
			);
		const contentLineCount = this.content?.trim().split('\n').length;
		const status = added == contentLineCount ? 'A' : removed == contentLineCount ? 'D' : 'M';
		return { status, added, removed };
	}
}

export class Branch {
	id!: string;
	name!: string;
	notes!: string;
	active!: boolean;
	@Type(() => File)
	files!: File[];
	@Type(() => Commit)
	commits!: Commit[];
	description!: string;
	mergeable!: boolean;
	mergeConflicts!: string[];
	order!: number;
	upstream?: string;
	conflicted!: boolean;
	baseCurrent!: boolean;
	ownership!: string;
}

export class Commit {
	id!: string;
	author!: Author;
	description!: string;
	@Transform((obj) => new Date(obj.value))
	createdAt!: Date;
	isRemote!: boolean;
	isIntegrated!: boolean;
	@Type(() => File)
	files!: File[];
}

export class Author {
	email!: string;
	name!: string;
	@Transform((obj) => new Date(obj.value))
	gravatarUrl!: URL;
}

// TODO: For consistency change Ts suffix to At, and return milliseconds from back end
export class BranchData {
	sha!: string;
	name!: string;
	@Transform((obj) => new Date(obj.value * 1000))
	lastCommitTs!: Date;
	@Transform((obj) => new Date(obj.value * 1000))
	firstCommitTs!: Date;
	ahead!: number;
	behind!: number;
	upstream?: string;
	authors!: Author[];
	mergeable!: boolean;
	mergeConflicts!: string[];
	@Type(() => Commit)
	commits!: Commit[];
}

export class BaseBranch {
	branchName!: string;
	remoteName!: string;
	remoteUrl!: string;
	baseSha!: string;
	currentSha!: string;
	behind!: number;
	@Type(() => Commit)
	upstreamCommits!: Commit[];
	@Type(() => Commit)
	recentCommits!: Commit[];
	fetchedAt!: Date;

	get repoBaseUrl(): string {
		return this.remoteUrl.replace(':', '/').replace('git@', 'https://').replace('.git', '');
	}

	commitUrl(commitId: string): string | undefined {
		return `${this.repoBaseUrl}/commit/${commitId}`;
	}
}

export interface WritableReloadable<T> extends WritableLoadable<T> {
	state: Readable<LoadState>;
	reload(visitedMap?: VisitedMap): Promise<T>;
}
