import { getConflictState } from "$lib/files/conflictEntryPresence";
import type { ConflictState } from "$lib/files/conflictEntryPresence";
import type { FileService } from "$lib/files/fileService";
import type { ConflictEntryPresence } from "@gitbutler/but-sdk";

interface ConflictFile {
	path: string;
	conflictEntryPresence?: ConflictEntryPresence;
}

/**
 * Re-reads each conflicted file from disk and records its conflict state
 * in `states` as the reads land. A file that cannot be read counts as
 * "unknown" rather than failing the whole refresh.
 */
export function refreshConflictStates(
	files: ConflictFile[],
	fileService: FileService,
	projectId: string,
	states: Map<string, ConflictState>,
) {
	for (const file of files) {
		if (!file.conflictEntryPresence) continue;
		const presence = file.conflictEntryPresence;
		const path = file.path;
		fileService
			.readFromWorkspace(path, projectId)
			.then((info) => states.set(path, getConflictState(presence, info)))
			.catch(() => states.set(path, "unknown"));
	}
}

/**
 * Re-reads conflicted files from disk to check whether conflicts are
 * truly unresolved. The reactive UI state can lag behind actual file
 * contents, so this gives an authoritative answer at call time.
 *
 * A file whose state cannot be determined (unreadable, non-UTF-8, or
 * binary content) counts as unresolved; marking it resolved by hand is
 * how the user overrules that.
 */
export async function hasUnresolvedConflictsOnDisk(
	files: ConflictFile[],
	manuallyResolved: ReadonlySet<string>,
	fileService: FileService,
	projectId: string,
): Promise<boolean> {
	for (const file of files) {
		if (!file.conflictEntryPresence) continue;
		if (manuallyResolved.has(file.path)) continue;
		const info = await fileService.readFromWorkspace(file.path, projectId).catch(() => undefined);
		const state = getConflictState(file.conflictEntryPresence, info);
		if (state !== "resolved") {
			return true;
		}
	}
	return false;
}
