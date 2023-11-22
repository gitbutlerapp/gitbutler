import 'reflect-metadata';
import { Type, Transform } from 'class-transformer';

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
	requiresForce!: boolean;
	description!: string;
	order!: number;
	@Type(() => RemoteBranch)
	upstream?: RemoteBranch;
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
	parentIds!: string[];
	branchId!: string;
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

export interface Author {
	email: string;
	name: string;
	gravatarUrl?: URL;
	isBot: boolean;
}

export class RemoteBranch {
	sha!: string;
	name!: string;
	upstream?: string;
	behind!: number;
	@Type(() => RemoteCommit)
	commits!: RemoteCommit[];
	isMergeable!: boolean | undefined;

	get ahead(): number {
		return this.commits.length;
	}

	get lastCommitTs(): Date {
		return this.commits[0].createdAt;
	}

	get firstCommitAt(): Date {
		return this.commits[this.commits.length - 1].createdAt;
	}

	get authors(): Author[] {
		const allAuthors = this.commits.map((commit) => commit.author);
		const uniqueAuthors = allAuthors.filter(
			(author, index) => allAuthors.findIndex((a) => a.email == author.email) == index
		);
		return uniqueAuthors;
	}

	get displayName(): string {
		return this.name.replace('refs/remotes/', '').replace('origin/', '').replace('refs/heads/', '');
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
