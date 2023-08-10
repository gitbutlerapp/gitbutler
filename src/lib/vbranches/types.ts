import 'reflect-metadata';
import { Type, Transform } from 'class-transformer';
import type { Readable, WritableLoadable } from '@square/svelte-store';
import type { LoadState, VisitedMap } from '@square/svelte-store/lib/async-stores/types';

export class Hunk {
	id!: string;
	diff!: string;
	@Transform((obj) => {
		return new Date(obj.value);
	})
	modifiedAt!: Date;
	filePath!: string;
	locked!: boolean;
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
}

export class Branch {
	id!: string;
	name!: string;
	active!: boolean;
	@Type(() => File)
	files!: File[];
	commits!: Commit[];
	description!: string;
	mergeable!: boolean;
	mergeConflicts!: string[];
	order!: number;
	upstream?: string;
	conflicted!: boolean;
	baseCurrent!: boolean;
	integrated!: boolean;
}

export class Commit {
	id!: string;
	author!: Author;
	description!: string;
	@Transform((obj) => new Date(obj.value))
	createdAt!: Date;
	isRemote!: boolean;
	@Type(() => File)
	files!: File[];
}

export class Author {
	email!: string;
	name!: string;
	@Transform((obj) => new Date(obj.value))
	gravatarUrl!: URL;
}

export class BranchData {
	sha!: string;
	name!: string;
	lastCommitTs!: number;
	firstCommitTs!: number;
	ahead!: number;
	behind!: number;
	upstream?: string;
	authors!: Author[];
	mergeable!: boolean;
	mergeConflicts!: string[];
	commits!: Commit[];
}

export class BaseBranch {
	branchName!: string;
	remoteName!: string;
	remoteUrl!: string;
	baseSha!: string;
	currentSha!: string;
	behind!: number;
	upstreamCommits!: Commit[];
	recentCommits!: Commit[];

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
