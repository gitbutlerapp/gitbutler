/**
 * Utilities for determining file types from TreeChange objects and ChangeState
 */

export type FileType = 'regular' | 'executable' | 'symlink' | 'submodule';

/**
 * Determines the file type from a ChangeState object or status
 */
export function getFileType(status: any): FileType {
	// Handle the different status types: Addition, Deletion, Modification, Rename
	const state = getRelevantState(status);
	
	if (!state || !state.kind) {
		return 'regular';
	}

	switch (state.kind) {
		case 'Link':
			return 'symlink';
		case 'Commit':
			return 'submodule';
		case 'BlobExecutable':
			return 'executable';
		case 'Blob':
		default:
			return 'regular';
	}
}

/**
 * Gets the relevant state from a status object
 */
function getRelevantState(status: any): any {
	if (!status || !status.subject) {
		return null;
	}

	const subject = status.subject;

	// For additions, use the state
	if (subject.state) {
		return subject.state;
	}

	// For deletions, we might need the previous state
	if (subject.previousState) {
		return subject.previousState;
	}

	return null;
}

/**
 * Checks if a file is executable (but not a symlink or submodule)
 */
export function isExecutableFile(status: any): boolean {
	const fileType = getFileType(status);
	return fileType === 'executable';
}

/**
 * Checks if a file is a symlink
 */
export function isSymlink(status: any): boolean {
	const fileType = getFileType(status);
	return fileType === 'symlink';
}

/**
 * Checks if a file is a submodule
 */
export function isSubmodule(status: any): boolean {
	const fileType = getFileType(status);
	return fileType === 'submodule';
}

/**
 * Checks if a file should show an executable overlay
 * (executable files that are not symlinks or submodules)
 */
export function shouldShowExecutableOverlay(status: any): boolean {
	const fileType = getFileType(status);
	return fileType === 'executable';
}