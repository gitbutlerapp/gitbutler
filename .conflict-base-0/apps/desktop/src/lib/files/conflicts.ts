import 'reflect-metadata';
import { emptyConflictEntryPresence, type ConflictEntryPresence } from '$lib/conflictEntryPresence';

export class ConflictEntries {
	public entries: Map<string, ConflictEntryPresence> = new Map();
	constructor(ancestorEntries: string[], ourEntries: string[], theirEntries: string[]) {
		ancestorEntries.forEach((entry) => {
			const entryPresence = this.entries.get(entry) || emptyConflictEntryPresence();
			entryPresence.ancestor = true;
			this.entries.set(entry, entryPresence);
		});
		ourEntries.forEach((entry) => {
			const entryPresence = this.entries.get(entry) || emptyConflictEntryPresence();
			entryPresence.ours = true;
			this.entries.set(entry, entryPresence);
		});
		theirEntries.forEach((entry) => {
			const entryPresence = this.entries.get(entry) || emptyConflictEntryPresence();
			entryPresence.theirs = true;
			this.entries.set(entry, entryPresence);
		});
	}
}
