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

function looksConflicted(file: string): boolean {
	const lines = file.split('\n');
	for (const line of lines) {
		if (line.startsWith('<<<<<<<')) {
			return true;
		}
	}
	return false;
}

export type ConflictState = 'conflicted' | 'resolved' | 'unknown';

export function getConflictState(
	conflictEntryPresence: ConflictEntryPresence,
	file: string
): ConflictState {
	if (!conflictEntryPresence.ours || !conflictEntryPresence.theirs) {
		return 'conflicted';
	}

	if (looksConflicted(file)) {
		return 'conflicted';
	}

	return 'resolved';
}
