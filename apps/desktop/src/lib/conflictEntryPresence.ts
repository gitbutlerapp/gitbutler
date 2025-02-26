import type { RemoteFile } from './files/file';
import type { RemoteHunk } from './hunks/hunk';
import type { FileStatus } from './utils/fileStatus';

export interface ConflictEntryPresence {
	ours: boolean;
	theirs: boolean;
	ancestor: boolean;
}

export function emptyConflictEntryPresence(): ConflictEntryPresence {
	return {
		ancestor: false,
		ours: false,
		theirs: false
	};
}

export function conflictEntryHint(presence: ConflictEntryPresence): string {
	let defaultVerb = 'added';

	if (presence.ancestor) {
		defaultVerb = 'modified';
	}

	let oursVerb = defaultVerb;

	if (!presence.ours) {
		oursVerb = 'deleted';
	}

	let theirsVerb = defaultVerb;

	if (!presence.theirs) {
		theirsVerb = 'deleted';
	}

	return `You have ${theirsVerb} this file, They have ${oursVerb} this file.`;
}

function hunkLooksConflicted(hunk: RemoteHunk): boolean {
	const lines = hunk.diff.split('\n');
	for (const line of lines) {
		if (line.startsWith('+<<<<<<<')) {
			return true;
		}
	}
	return false;
}

export type ConflictState = 'conflicted' | 'resolved' | 'unknown';

export function getConflictState(
	file: RemoteFile,
	conflictEntryPresence: ConflictEntryPresence
): ConflictState {
	if (!conflictEntryPresence.ours || !conflictEntryPresence.theirs) {
		return 'unknown';
	}

	for (const hunk of file.hunks) {
		if (hunkLooksConflicted(hunk)) {
			return 'conflicted';
		}
	}
	return 'resolved';
}

export function getInitialFileStatus(
	uncommitedFileChange: RemoteFile | undefined,
	conflictEntryPresence: ConflictEntryPresence | undefined
): FileStatus | undefined {
	if (!conflictEntryPresence) {
		return undefined;
	}

	if (!uncommitedFileChange) {
		// If there is a conflict, resolving using ours would show as no file present
		return 'M';
	}

	const conflictState = getConflictState(uncommitedFileChange, conflictEntryPresence);
	return conflictState === 'resolved' ? 'M' : undefined;
}
