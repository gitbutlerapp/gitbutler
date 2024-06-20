/**
 * Shared helper functions for manipulating selected files with keyboard.
 */
import { getSelectionDirection } from './getSelectionDirection';
import { stringifyFileKey, unstringifyFileKey } from '$lib/vbranches/fileIdSelection';
import type { FileIdSelection } from '$lib/vbranches/fileIdSelection';
import type { AnyFile } from '$lib/vbranches/types';

export function getNextFile(files: AnyFile[], currentId: string): AnyFile | undefined {
	const fileIndex = files.findIndex((f) => f.id === currentId);
	return fileIndex !== -1 && fileIndex + 1 < files.length ? files[fileIndex + 1] : undefined;
}

export function getPreviousFile(files: AnyFile[], currentId: string): AnyFile | undefined {
	const fileIndex = files.findIndex((f) => f.id === currentId);
	return fileIndex > 0 ? files[fileIndex - 1] : undefined;
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
	if (selected && selected.fileId === file.id && selected.commitId === commitId) {
		elt.focus();
	}
}

export function maybeMoveSelection(
	allowMultiple: boolean,
	shiftKey: boolean,
	key: string,
	file: AnyFile,
	files: AnyFile[],
	selectedFileIds: string[],
	fileIdSelection: FileIdSelection
) {
	if (selectedFileIds.length === 0) return;

	const firstFileId = unstringifyFileKey(selectedFileIds[0]);
	const lastFileId = unstringifyFileKey(selectedFileIds[selectedFileIds.length - 1]);
	let selectionDirection = getSelectionDirection(
		files.findIndex((f) => f.id === lastFileId),
		files.findIndex((f) => f.id === firstFileId)
	);

	function getAndAddFile(
		getFileFunc: (files: AnyFile[], id: string) => AnyFile | undefined,
		id: string
	) {
		const file = getFileFunc(files, id);
		if (file) {
			// if file is already selected, do nothing
			if (selectedFileIds.includes(stringifyFileKey(file.id))) return;

			fileIdSelection.add(file.id);
		}
	}

	function getAndClearAndAddFile(
		getFileFunc: (files: AnyFile[], id: string) => AnyFile | undefined,
		id: string
	) {
		const file = getFileFunc(files, id);
		if (file) {
			fileIdSelection.clear();
			fileIdSelection.add(file.id);
		}
	}

	switch (key) {
		case 'ArrowUp':
			if (shiftKey && allowMultiple) {
				// Handle case if only one file is selected
				// we should update the selection direction
				if (selectedFileIds.length === 1) {
					selectionDirection = 'up';
				} else if (selectionDirection === 'down') {
					fileIdSelection.remove(lastFileId);
				}
				getAndAddFile(getPreviousFile, lastFileId);
			} else {
				// Handle reset of selection
				if (selectedFileIds.length > 1) {
					getAndClearAndAddFile(getPreviousFile, lastFileId);
				} else {
					getAndClearAndAddFile(getPreviousFile, file.id);
				}
			}
			break;

		case 'ArrowDown':
			if (shiftKey && allowMultiple) {
				// Handle case if only one file is selected
				// we should update the selection direction
				if (selectedFileIds.length === 1) {
					selectionDirection = 'down';
				} else if (selectionDirection === 'up') {
					fileIdSelection.remove(lastFileId);
				}
				getAndAddFile(getNextFile, lastFileId);
			} else {
				// Handle reset of selection
				if (selectedFileIds.length > 1) {
					getAndClearAndAddFile(getNextFile, lastFileId);
				} else {
					getAndClearAndAddFile(getNextFile, file.id);
				}
			}
			break;
	}
}
