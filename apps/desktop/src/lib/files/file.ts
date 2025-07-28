import { Hunk, type HunkLock } from '$lib/hunks/hunk';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import { Transform, Type } from 'class-transformer';
import 'reflect-metadata';

export type FileInfo = {
	content: string;
	name?: string;
	mimeType?: string;
	size?: number;
};
export class LocalFile {
	id!: string;
	path!: string;
	@Type(() => Hunk)
	hunks!: Hunk[];
	expanded?: boolean;
	@Transform((obj) => new Date(obj.value))
	modifiedAt!: Date;
	// This indicates if a file has merge conflict markers generated and not yet resolved.
	// This is true for files after a branch which does not apply cleanly (Branch.isMergeable === false) is applied.
	// (therefore this field is applicable only for the workspace, i.e. active === true)
	conflicted!: boolean;
	content!: string;
	binary!: boolean;
	large!: boolean;

	get filename(): string {
		const parts = this.path.split('/');
		return parts.at(-1) ?? this.path;
	}

	get justpath() {
		return this.path.split('/').slice(0, -1).join('/');
	}

	get hunkIds() {
		return this.hunks.map((h) => h.id);
	}

	get locked(): boolean {
		return this.hunks
			? this.hunks.map((hunk) => hunk.locked).reduce((a, b) => !!(a || b), false)
			: false;
	}

	get lockedIds(): HunkLock[] {
		return this.hunks.flatMap((hunk) => hunk.lockedTo).filter(isDefined);
	}
}
export type AnyFile = LocalFile;

export function isAnyFile(something: unknown): something is AnyFile {
	return something instanceof LocalFile;
}
