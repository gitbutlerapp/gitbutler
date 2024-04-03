/**
 * Shared helper functions for manipulating selected files with keyboard.
 */
import { get, type Writable } from 'svelte/store';
import type { FileSelection } from '$lib/vbranches/fileSelection';
import type { AnyFile } from '$lib/vbranches/types';

export function getNextFile(files: AnyFile[], current: string) {
	const fileIndex = files.findIndex((f) => f.id == current);
	if (fileIndex != -1 && fileIndex + 1 < files.length) return files[fileIndex + 1];
}

export function getPreviousFile(files: AnyFile[], current: string) {
	const fileIndex = files.findIndex((f) => f.id == current);
	if (fileIndex > 0) return files[fileIndex - 1];
}

export function getFileByKey(key: string, current: string, files: AnyFile[]): AnyFile | undefined {
	if (key == 'ArrowUp') {
		return getPreviousFile(files, current);
	} else if (key == 'ArrowDown') {
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
	selectedFiles: Writable<FileSelection>,
	commitId?: string
) {
	const selection = get(selectedFiles);

	if (selection.length != 1) return;
	const selected = selection.toOnly();
	if (selected.fileId == file.id && selected.context == commitId) elt.focus();
}

export function maybeMoveSelection(
	key: string,
	files: AnyFile[],
	selectedFileIds: Writable<FileSelection>
) {
	if (key != 'ArrowUp' && key != 'ArrowDown') return;

	const currentSelection = get(selectedFileIds);
	if (currentSelection.length == 0) return;

	const newSelection = getFileByKey(key, currentSelection.toOnly().fileId, files);
	if (newSelection) {
		currentSelection.clear();
		currentSelection.add(newSelection.id);
	}
}
