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

interface MoveSelectionParams {
	allowMultiple: boolean;
	shiftKey: boolean;
	key: string;
	targetElement: HTMLElement;
	file: AnyFile;
	files: AnyFile[];
	selectedFileIds: string[];
	fileIdSelection: FileIdSelection;
	commitId?: string;
}

export function maybeMoveSelection({
	allowMultiple,
	shiftKey,
	key,
	targetElement,
	file,
	files,
	selectedFileIds,
	fileIdSelection,
	commitId
}: MoveSelectionParams) {
	if (!selectedFileIds[0] || selectedFileIds.length === 0) return;

	const firstFileId = unstringifyFileKey(selectedFileIds[0]);
	const lastFileId = unstringifyFileKey(selectedFileIds.at(-1)!);
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

			if (selectedFileIds.includes(stringifyFileKey(file.id, commitId))) return;

			fileIdSelection.add(file.id, commitId);
		}
	}

	function getAndClearAndAddFile(
		getFileFunc: (files: AnyFile[], id: string) => AnyFile | undefined,
		id: string
	) {
		const file = getFileFunc(files, id);

		if (file) {
			fileIdSelection.clearExcept(file.id, commitId);
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
					fileIdSelection.remove(lastFileId, commitId);
				}
				getAndAddFile(getPreviousFile, lastFileId);
			} else {
				// focus previous file
				const previousElement = targetElement.previousElementSibling as HTMLElement;
				if (previousElement) previousElement.focus();

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
					fileIdSelection.remove(lastFileId, commitId);
				}

				getAndAddFile(getNextFile, lastFileId);
			} else {
				// focus next file
				const nextElement = targetElement.nextElementSibling as HTMLElement;
				if (nextElement) nextElement.focus();

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
