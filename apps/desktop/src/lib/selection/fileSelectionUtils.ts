/**
 * Shared helper functions for manipulating selected files with keyboard.
 *
 * This file replaces `$lib/utils/selection.ts`, with the main difference
 * being the type change from `AnyFile` to `TreeChange`.
 */
import { type SelectedFile, type SelectionId } from '$lib/selection/key';
import { getSelectionDirection } from '$lib/utils/getSelectionDirection';
import { KeyName } from '@gitbutler/ui/utils/hotkeys';
import { get } from 'svelte/store';
import type { TreeChange } from '$lib/hunks/change';
import type { FileSelectionManager } from '$lib/selection/fileSelectionManager.svelte';

function getFile(files: TreeChange[], id: string): TreeChange | undefined {
	return files.find((f) => f.path === id);
}

function getNextFile(files: TreeChange[], currentId: string): TreeChange | undefined {
	const fileIndex = files.findIndex((f) => f.path === currentId);
	if (fileIndex === -1) return undefined;

	const nextFileIndex = fileIndex + 1;
	return files[nextFileIndex];
}

function getPreviousFile(files: TreeChange[], currentId: string): TreeChange | undefined {
	const fileIndex = files.findIndex((f) => f.path === currentId);
	if (fileIndex === -1) return undefined;
	const previousFileIndex = fileIndex - 1;
	return files[previousFileIndex];
}

function getTopFile(files: TreeChange[], selectedFileIds: SelectedFile[]): TreeChange | undefined {
	for (const file of files) {
		if (selectedFileIds.find((f) => f.path === file.path)) {
			return file;
		}
	}
	return undefined;
}

function getBottomFile(
	files: TreeChange[],
	selectedFileIds: SelectedFile[]
): TreeChange | undefined {
	for (let i = files.length - 1; i >= 0; i--) {
		const file = files[i]!;
		if (selectedFileIds.find((f) => f.path === file.path)) {
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
	files: TreeChange[];
	selectedFileIds: SelectedFile[];
	fileIdSelection: FileSelectionManager;
	selectionId: SelectionId;
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
	selectionId,
	preventDefault
}: UpdateSelectionParams): boolean | undefined {
	if (!selectedFileIds[0] || selectedFileIds.length === 0) return;

	const firstFileId = selectedFileIds[0].path;
	const lastFileId = selectedFileIds.at(-1)!.path;

	const topFileId = getTopFile(files, selectedFileIds)?.path;
	const bottomFileId = getBottomFile(files, selectedFileIds)?.path;

	let selectionDirection = getSelectionDirection(
		files.findIndex((f) => f.path === lastFileId),
		files.findIndex((f) => f.path === firstFileId)
	);

	function getAndAddFile(
		id: string,
		getFileFunc?: (files: TreeChange[], id: string) => TreeChange | undefined
	) {
		const file = getFileFunc?.(files, id) ?? getFile(files, id);
		if (file) {
			const fileIndex = files.findIndex((f) => f.path === file.path);
			if (fileIndex === -1) return; // should never happen
			fileIdSelection.add(file.path, selectionId, fileIndex);
		}
	}

	function getAndClearExcept(
		id: string,
		getFileFunc?: (files: TreeChange[], id: string) => TreeChange | undefined
	) {
		const file = getFileFunc?.(files, id) ?? getFile(files, id);
		if (file) {
			const fileIndex = files.findIndex((f) => f.path === file.path);
			if (fileIndex === -1) return; // should never happen
			fileIdSelection.set(file.path, selectionId, fileIndex);
		}
	}

	switch (key) {
		case 'a':
			if (allowMultiple && metaKey) {
				preventDefault();
				for (let i = 0; i < files.length; i++) {
					const file = files[i]!;
					fileIdSelection.add(file.path, selectionId, i);
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
					fileIdSelection.remove(lastFileId, selectionId);
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
					fileIdSelection.remove(lastFileId, selectionId);
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
			fileIdSelection.clearPreview(selectionId);
			targetElement.blur();
			break;
	}
}

export function selectFilesInList(
	e: MouseEvent | KeyboardEvent,
	change: TreeChange,
	sortedFiles: TreeChange[],
	idSelection: FileSelectionManager,
	selectedFileIds: SelectedFile[],
	allowMultiple: boolean,
	index: number,
	selectionId: SelectionId,
	allowUnselect?: boolean
) {
	// e.stopPropagation();
	const isAlreadySelected = idSelection.has(change.path, selectionId);
	const isTheOnlyOneSelected = idSelection.collectionSize(selectionId) === 1 && isAlreadySelected;
	const lastAdded = get(idSelection.getById(selectionId).lastAdded);

	if (e.ctrlKey || e.metaKey) {
		if (isAlreadySelected) {
			idSelection.remove(change.path, selectionId);
			selectedFileIds.splice(selectedFileIds.findIndex((f) => f.path === change.path));
			const previous = selectedFileIds.at(-1);
			if (previous) {
				idSelection.add(previous.path, selectionId, selectedFileIds.length - 1);
			}
		} else {
			idSelection.add(change.path, selectionId, index);
		}
	} else if (e.shiftKey && allowMultiple && lastAdded !== undefined) {
		const start = Math.min(lastAdded.index, index);
		const end = Math.max(lastAdded.index, index);

		const filePaths = sortedFiles.slice(start, end + 1).map((f) => f.path);
		idSelection.addMany(filePaths, selectionId, { path: change.path, index });
	} else {
		// if only one file is selected and it is already selected, unselect it
		if (isTheOnlyOneSelected) {
			if (allowUnselect) {
				idSelection.clear(selectionId);
			}
		} else {
			idSelection.set(change.path, selectionId, index);
		}
	}
}
