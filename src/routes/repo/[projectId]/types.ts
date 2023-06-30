import { Transform, Type } from 'class-transformer';
import 'reflect-metadata';

class DndItem {
	id!: string;
}

export class Hunk extends DndItem {
	name!: string;
	diff!: string;
	@Transform((obj) => new Date(obj.value))
	modifiedAt!: Date;
	filePath!: string;
}

export class File extends DndItem {
	path!: string;
	@Type(() => Hunk)
	hunks!: Hunk[];
	expanded?: boolean;
}

export class Branch extends DndItem {
	name!: string;
	active!: boolean;
	@Type(() => File)
	files!: File[];
	commits!: Commit[];
	description!: string;
	mergeable!: boolean;
	mergeConflicts!: string[];
	order!: number;
}

export type BranchData = {
	sha: string;
	branch: string;
	name: string;
	description: string;
	lastCommitTs: number;
	firstCommitTs: number;
	ahead: number;
	behind: number;
	upstream: string;
	authors: string[];
	mergeable: boolean;
	mergeConflicts: string[];
};

export class Commit {
	id!: string;
	authorEmail!: string;
	authorName!: string;
	description!: string;
	@Transform((obj) => {
		return new Date(obj.value);
	})
	createdAt!: Date;
	isRemote!: boolean;
}

export class Target {
	sha!: string;
	name!: string;
	remote!: string;
	behind!: number;
}
