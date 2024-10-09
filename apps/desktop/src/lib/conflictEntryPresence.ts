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
