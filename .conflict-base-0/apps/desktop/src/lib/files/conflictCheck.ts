import { getConflictState } from "$lib/files/conflictEntryPresence";
import type { FileService } from "$lib/files/fileService";
import type { ConflictEntryPresence } from "@gitbutler/but-sdk";

interface ConflictFile {
	path: string;
	conflictEntryPresence?: ConflictEntryPresence;
}

/**
 * Re-reads conflicted files from disk to check whether conflicts are
 * truly unresolved. The reactive UI state can lag behind actual file
 * contents, so this gives an authoritative answer at call time.
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
		const result = await fileService.readFromWorkspace(file.path, projectId);
		const state = getConflictState(file.conflictEntryPresence, result.data.content);
		if (state === "conflicted") {
			return true;
		}
	}
	return false;
}
