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
	large!: boolean;
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
	order!: number;
	upstream?: string;
	@Type(() => Commit)
	upstreamCommits!: Commit[];
	conflicted!: boolean;
	baseCurrent!: boolean;
	ownership!: string;
	isMergeable!: Promise<boolean>;
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

export class RemoteCommit {
	id!: string;
	author!: Author;
	description!: string;
	@Transform((obj) => new Date(obj.value * 1000))
	createdAt!: Date;
}

export class RemoteHunk {
	diff!: string;
}

export class RemoteFile {
	path!: string;
	@Type(() => RemoteHunk)
	hunks!: RemoteHunk[];
	binary!: boolean;
}

export class Author {
	email!: string;
	name!: string;
	@Transform((obj) => new Date(obj.value))
	gravatarUrl!: URL;
}

export class RemoteBranch {
	sha!: string;
	name!: string;
	behind!: number;
	upstream?: string;
	@Type(() => RemoteCommit)
	commits!: RemoteCommit[];
	isMergeable!: Promise<boolean>;

	ahead(): number {
		return this.commits.length;
	}

	lastCommitTs(): Date | undefined {
		return this.commits.at(0)?.createdAt;
	}

	authors(): Author[] {
		const allAuthors = this.commits.map((commit) => commit.author);
		const uniqueAuthors = allAuthors.filter(
			(author, index) => allAuthors.findIndex((a) => a.email == author.email) == index
		);
		return uniqueAuthors;
	}
}

export class BaseBranch {
	branchName!: string;
	remoteName!: string;
	remoteUrl!: string;
	baseSha!: string;
	currentSha!: string;
	behind!: number;
	@Type(() => RemoteCommit)
	upstreamCommits!: RemoteCommit[];
	@Type(() => RemoteCommit)
	recentCommits!: RemoteCommit[];
	fetchedAt!: Date;

	get repoBaseUrl(): string {
		if (this.remoteUrl.startsWith('http')) {
			return this.remoteUrl.replace('.git', '');
		} else {
			return this.remoteUrl.replace(':', '/').replace('git@', 'https://').replace('.git', '');
		}
	}

	commitUrl(commitId: string): string | undefined {
		return `${this.repoBaseUrl}/commit/${commitId}`;
	}
}
export interface CustomStore<T> extends WritableLoadable<T> {
	state: Readable<LoadState>;
	reload(visitedMap?: VisitedMap): Promise<T>;
}

export interface VirtualBranchStore<T> extends CustomStore<T[] | undefined> {
	updateById(id: string, updater: (value: T) => void): void;
}
