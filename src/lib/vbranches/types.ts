import 'reflect-metadata';
import { Type, Transform } from 'class-transformer';

export class Hunk {
	id!: string;
	name!: string;
	diff!: string;
	@Transform((obj) => {
		return new Date(obj.value);
	})
	modifiedAt!: Date;
	filePath!: string;
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
	upstream!: string;
	conflicted!: boolean;
	baseCurrent!: boolean;
}

export class Commit {
	id!: string;
	authorEmail!: string;
	authorName!: string;
	description!: string;
	@Transform((obj) => new Date(obj.value))
	createdAt!: Date;
	isRemote!: boolean;
}

export class BranchData {
	sha!: string;
	branch!: string;
	name!: string;
	description!: string;
	lastCommitTs!: number;
	firstCommitTs!: number;
	ahead!: number;
	behind!: number;
	upstream!: string;
	authors!: string[];
	mergeable!: boolean;
	mergeConflicts!: string[];
}

export class Target {
	sha!: string;
	branchName!: string;
	remoteName!: string;
	remoteUrl!: string;
	behind!: number;
}
