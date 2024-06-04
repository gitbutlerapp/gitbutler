/**
 * Shared helper functions for manipulating selected files with keyboard.
 */
import type { FileIdSelection } from '$lib/vbranches/fileIdSelection';
import type { AnyFile } from '$lib/vbranches/types';

export function getNextFile(files: AnyFile[], current: string) {
	const fileIndex = files.findIndex((f) => f.id === current);
	if (fileIndex !== -1 && fileIndex + 1 < files.length) return files[fileIndex + 1];
}

export function getPreviousFile(files: AnyFile[], current: string) {
	const fileIndex = files.findIndex((f) => f.id === current);
	if (fileIndex > 0) return files[fileIndex - 1];
}

export function getFileByKey(key: string, current: string, files: AnyFile[]): AnyFile | undefined {
	if (key === 'ArrowUp') {
		return getPreviousFile(files, current);
	} else if (key === 'ArrowDown') {
		return getNextFile(files, current);
	}
}

/**
 * When the selectedFiles store updates we run this function for every rendered file to determine
 * if it is the only selected file. If so we focus the containing element.
 *
 * This has potential to slow things down since it's O(N) wrt to number of files, but it's less
 * prone to breakage than focusing using element ids at a distance.
 */
export function updateFocus(
	elt: HTMLElement,
	file: AnyFile,
	fileIdSelection: FileIdSelection,
	commitId?: string
) {
	const selected = fileIdSelection.only();
	if (!selected) return;
	if (selected.fileId === file.id && selected.commitId === commitId) elt.focus();
}

export function maybeMoveSelection(
	key: string,
	file: AnyFile,
	files: AnyFile[],
	fileIdSelection: FileIdSelection
) {
	if (key !== 'ArrowUp' && key !== 'ArrowDown') return;

	const newSelection = getFileByKey(key, file.id, files);
	if (newSelection) {
		fileIdSelection.clear();
		fileIdSelection.add(newSelection.id);
	}
}
