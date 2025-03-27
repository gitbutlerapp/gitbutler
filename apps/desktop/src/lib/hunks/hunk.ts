import { hashCode } from '@gitbutler/ui/utils/string';
import { Transform, Type } from 'class-transformer';
import type { Prettify } from '@gitbutler/shared/utils/typeUtils';
import 'reflect-metadata';

export class RemoteHunk {
	diff!: string;
	hash?: string;
	oldStart!: number;
	oldLines!: number;
	newStart!: number;
	newLines!: number;
	changeType!: ChangeType;

	get id(): string {
		return hashCode(this.diff);
	}

	get locked() {
		return false;
	}
}
export type ChangeType =
	/// Entry does not exist in old version
	| 'added'
	/// Entry is untracked item in workdir
	| 'untracked'
	/// Entry does not exist in new version
	| 'deleted'
	/// Entry content changed between old and new
	| 'modified';

export class Hunk {
	id!: string;
	diff!: string;
	@Transform((obj) => {
		return new Date(obj.value);
	})
	modifiedAt!: Date;
	filePath!: string;
	hash?: string;
	locked!: boolean;
	@Type(() => HunkLock)
	lockedTo!: HunkLock[];
	/// Indicates that the hunk depends on multiple branches. In this case the hunk cant be moved or comitted.
	poisoned!: boolean;
	changeType!: ChangeType;
	oldStart!: number;
	oldLines!: number;
	newStart!: number;
	newLines!: number;
}

export class HunkLock {
	branchId!: string;
	commitId!: string;
}

export type DiffSpec = {
	/** lossless version of `previous_path` if this was a rename. */
	readonly previousPathBytes: string | null;
	/** lossless version of `path`. */
	readonly pathBytes: string;
	/** The headers of the hunks to use, or empty if all changes are to be used. */
	readonly hunkHeaders: HunkHeader[];
};

export type HunkHeader = {
	/** The 1-based line number at which the previous version of the file started.*/
	readonly oldStart: number;
	/** The non-zero amount of lines included in the previous version of the file.*/
	readonly oldLines: number;
	/** The 1-based line number at which the new version of the file started.*/
	readonly newStart: number;
	/** The non-zero amount of lines included in the new version of the file.*/
	readonly newLines: number;
};

/** A hunk as used in UnifiedDiff. */
export type DiffHunk = Prettify<
	HunkHeader & {
		/**
		 * A unified-diff formatted patch like:
		 *
		 * ```diff
		 * @@ -1,6 +1,8 @@
		 * This is the first line of the original text.
		 * -Line to be removed.
		 * +Line that has been replaced.
		 * This is another line in the file.
		 * +This is a new line added at the end.
		 * ```
		 *
		 * The line separator is the one used in the original file and may be `LF` or `CRLF`.
		 * Note that the file-portion of the header isn't used here.
		 */
		readonly diff: string;
	}
>;

export function isDiffHunk(something: unknown): something is DiffHunk {
	return (
		typeof something === 'object' &&
		something !== null &&
		'oldStart' in something &&
		typeof (something as any).oldStart === 'number' &&
		'oldLines' in something &&
		typeof (something as any).oldLines === 'number' &&
		'newStart' in something &&
		typeof (something as any).newStart === 'number' &&
		'newLines' in something &&
		typeof (something as any).newLines === 'number' &&
		'diff' in something &&
		typeof (something as any).diff === 'string'
	);
}

/**
 * A patch that if applied to the previous state of the resource would yield the current state.
 * Includes all non-overlapping hunks, including their context lines.
 */
export type Patch = {
	/** All non-overlapping hunks, including their context lines. */
	readonly hunks: DiffHunk[];
};
