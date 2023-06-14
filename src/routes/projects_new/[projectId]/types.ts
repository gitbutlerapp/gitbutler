import { Type } from 'class-transformer';
import 'reflect-metadata';

class DndItem {
	id!: string;
	kind!: string;
	isDndShadowItem?: boolean;
}

export class Hunk extends DndItem {
	name!: string;
	diff!: string;
	modifiedAt!: Date;
	filePath!: string;
}

export class File extends DndItem {
	path!: string;
	@Type(() => Hunk)
	hunks!: Hunk[];
}

export class Commit extends DndItem {
	description?: string;
	committedAt?: Date;
	@Type(() => File)
	files!: File[];
}

export class Branch extends DndItem {
	name!: string;
	active!: boolean;
	@Type(() => Commit)
	commits!: Commit[];
}
