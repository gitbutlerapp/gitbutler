/**
 * Shared helper functions for manipulating selected files with keyboard.
 *
 * TODO: Remove this file once V3 has shipped.
 */
import { getSelectionDirection } from './getSelectionDirection';
import { KeyName } from './hotkeys';
import { stringifyKey, unstringifyFileKey } from '$lib/selection/fileIdSelection';
import type { AnyFile } from '$lib/files/file';
import type { FileIdSelection } from '$lib/selection/fileIdSelection';

function getFile(files: AnyFile[], id: string): AnyFile | undefined {
	return files.find((f) => f.id === id);
}

function getNextFile(files: AnyFile[], currentId: string): AnyFile | undefined {
	const fileIndex = files.findIndex((f) => f.id === currentId);
	return fileIndex !== -1 && fileIndex + 1 < files.length ? files[fileIndex + 1] : undefined;
}

function getPreviousFile(files: AnyFile[], currentId: string): AnyFile | undefined {
	const fileIndex = files.findIndex((f) => f.id === currentId);
	return fileIndex > 0 ? files[fileIndex - 1] : undefined;
}

function getTopFile(files: AnyFile[], selectedFileIds: string[]): AnyFile | undefined {
	for (const file of files) {
		if (selectedFileIds.includes(stringifyKey(file.id))) {
			return file;
		}
	}
	return undefined;
}

function getBottomFile(files: AnyFile[], selectedFileIds: string[]): AnyFile | undefined {
	for (let i = files.length - 1; i >= 0; i--) {
		const file = files[i];
		if (selectedFileIds.includes(stringifyKey(file!.id))) {
			return file;
		}
	}
	return undefined;
}

interface UpdateSelectionParams {
	allowMultiple: boolean;
	metaKey: boolean;
	shiftKey: boolean;
	key: string;
	targetElement: HTMLElement;
	files: AnyFile[];
	selectedFileIds: string[];
	fileIdSelection: FileIdSelection;
	commitId?: string;
	preventDefault: () => void;
}

export function updateSelection({
	allowMultiple,
	metaKey,
	shiftKey,
	key,
	targetElement,
	files,
	selectedFileIds,
	fileIdSelection,
	commitId,
	preventDefault
}: UpdateSelectionParams) {
	if (!selectedFileIds[0] || selectedFileIds.length === 0) return;

	const firstFileId = unstringifyFileKey(selectedFileIds[0]);
	const lastFileId = unstringifyFileKey(selectedFileIds.at(-1)!);

	const topFileId = getTopFile(files, selectedFileIds)?.id;
	const bottomFileId = getBottomFile(files, selectedFileIds)?.id;

	let selectionDirection = getSelectionDirection(
		files.findIndex((f) => f.id === lastFileId),
		files.findIndex((f) => f.id === firstFileId)
	);

	function getAndAddFile(
		id: string,
		getFileFunc?: (files: AnyFile[], id: string) => AnyFile | undefined
	) {
		const file = getFileFunc?.(files, id) ?? getFile(files, id);
		if (file) {
			// if file is already selected, do nothing
			if (selectedFileIds.includes(stringifyKey(file.id, commitId))) return;

			fileIdSelection.add(file, commitId);
		}
	}

	function getAndClearExcept(
		id: string,
		getFileFunc?: (files: AnyFile[], id: string) => AnyFile | undefined
	) {
		const file = getFileFunc?.(files, id) ?? getFile(files, id);
		if (file) {
			fileIdSelection.set(file, commitId);
		}
	}

	switch (key) {
		case 'a':
			if (allowMultiple && metaKey) {
				preventDefault();
				for (const file of files) {
					fileIdSelection.add(file, commitId);
				}
			}
			break;
		case 'k':
		case KeyName.Up:
			preventDefault();
			if (shiftKey && allowMultiple) {
				// Handle case if only one file is selected
				// we should update the selection direction
				if (selectedFileIds.length === 1) {
					selectionDirection = 'up';
				} else if (selectionDirection === 'down') {
					fileIdSelection.remove(lastFileId, commitId);
				}
				getAndAddFile(lastFileId, getPreviousFile);
			} else {
				// Handle reset of selection
				if (selectedFileIds.length > 1 && topFileId !== undefined) {
					getAndClearExcept(topFileId);
				}

				// Handle navigation
				if (selectedFileIds.length === 1) {
					getAndClearExcept(firstFileId, getPreviousFile);
				}
			}
			break;

		case 'j':
		case KeyName.Down:
			preventDefault();
			if (shiftKey && allowMultiple) {
				// Handle case if only one file is selected
				// we should update the selection direction
				if (selectedFileIds.length === 1) {
					selectionDirection = 'down';
				} else if (selectionDirection === 'up') {
					fileIdSelection.remove(lastFileId, commitId);
				}

				getAndAddFile(lastFileId, getNextFile);
			} else {
				// Handle reset of selection
				if (selectedFileIds.length > 1 && bottomFileId !== undefined) {
					getAndClearExcept(bottomFileId);
				}

				// Handle navigation
				if (selectedFileIds.length === 1) {
					getAndClearExcept(firstFileId, getNextFile);
				}
			}
			break;
		case KeyName.Escape:
			preventDefault();
			fileIdSelection.clear();
			targetElement.blur();
			break;
	}
}
