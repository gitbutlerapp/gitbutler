/**
 * Shared helper functions for manipulating selected files with keyboard.
 */
import { get, type Writable } from 'svelte/store';
import type { AnyFile } from '$lib/vbranches/types';

export function getNextFile(files: AnyFile[], current: AnyFile) {
	const fileIndex = files.findIndex((f) => f.id == current.id);
	if (fileIndex != -1 && fileIndex + 1 < files.length) return files[fileIndex + 1];
}

export function getPreviousFile(files: AnyFile[], current: AnyFile) {
	const fileIndex = files.findIndex((f) => f.id == current.id);
	if (fileIndex > 0) return files[fileIndex - 1];
}

export function getFileByKey(key: string, current: AnyFile, files: AnyFile[]): AnyFile | undefined {
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
export function updateFocus(elt: HTMLElement, file: AnyFile, selectedFiles: Writable<AnyFile[]>) {
	const selection = get(selectedFiles);
	if (selection.length == 1 && selection[0].id == file.id) elt.focus();
}

export function maybeMoveSelection(
	key: string,
	files: AnyFile[],
	selectedFiles: Writable<AnyFile[]>
) {
	if (key != 'ArrowUp' && key != 'ArrowDown') return;

	const current = get(selectedFiles).at(0);
	if (!current) return;

	const newSelection = getFileByKey(key, current, files);
	if (newSelection) selectedFiles.set([newSelection]);
}
