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
}

export class VCommit {
	sha!: string;
	message!: string;
	timestamp!: Date;
	name!: string;
	email!: string;
}

export class Branch extends DndItem {
	name!: string;
	active!: boolean;
	@Type(() => File)
	files!: File[];
	commits!: VCommit[];
	description!: string;
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
};
