/**
 * Shared helper functions for manipulating selected files with keyboard.
 *
 * This file replaces `$lib/utils/selection.ts`, with the main difference
 * being the type change from `AnyFile` to `TreeChange`.
 */
import { getSelectionDirection } from '$lib/utils/getSelectionDirection';
import { KeyName } from '$lib/utils/hotkeys';
import type { TreeChange } from '$lib/hunks/change';
import type { IdSelection } from './idSelection.svelte';
import type { SelectedFile } from './key';

function getFile(files: TreeChange[], id: string): TreeChange | undefined {
	return files.find((f) => f.path === id);
}

function getNextFile(files: TreeChange[], currentId: string): TreeChange | undefined {
	const fileIndex = files.findIndex((f) => f.path === currentId);
	return fileIndex !== -1 && fileIndex + 1 < files.length ? files[fileIndex + 1] : undefined;
}

function getPreviousFile(files: TreeChange[], currentId: string): TreeChange | undefined {
	const fileIndex = files.findIndex((f) => f.path === currentId);
	return fileIndex > 0 ? files[fileIndex - 1] : undefined;
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
	fileIdSelection: IdSelection;
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
			// if file is already selected, do nothing
			if (selectedFileIds.find((f) => f.path === file.path)) {
				return;
			}

			fileIdSelection.add(file.path, commitId);
		}
	}

	function getAndClearExcept(
		id: string,
		getFileFunc?: (files: TreeChange[], id: string) => TreeChange | undefined
	) {
		const file = getFileFunc?.(files, id) ?? getFile(files, id);
		if (file) {
			fileIdSelection.set(file.path, commitId);
		}
	}

	switch (key) {
		case 'a':
			if (allowMultiple && metaKey) {
				preventDefault();
				for (const file of files) {
					fileIdSelection.add(file.path, commitId);
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

export function selectFilesInList(
	e: MouseEvent,
	change: TreeChange,
	sortedFiles: TreeChange[],
	idSelection: IdSelection,
	allowMultiple: boolean,
	commitId: string | undefined
) {
	e.stopPropagation();
	const isAlreadySelected = idSelection.has(change.path, commitId);

	if (e.ctrlKey || e.metaKey) {
		if (isAlreadySelected) {
			idSelection.remove(change.path, commitId);
		} else {
			idSelection.add(change.path, commitId);
		}
	} else if (e.shiftKey && allowMultiple) {
		// TODO(CTO): Not sure that this is accurate.
		// TODO(mattias): I think you're right.
		const initiallySelectedIndex = sortedFiles.findIndex((f) => f.path === idSelection.firstPath());

		// detect the direction of the selection
		const selectionDirection = getSelectionDirection(
			initiallySelectedIndex,
			sortedFiles.findIndex((f) => f.path === change.path)
		);

		const updatedSelection = sortedFiles.slice(
			Math.min(
				initiallySelectedIndex,
				sortedFiles.findIndex((f) => f.path === change.path)
			),
			Math.max(
				initiallySelectedIndex,
				sortedFiles.findIndex((f) => f.path === change.path)
			) + 1
		);

		// if the selection is in the opposite direction, reverse the selection
		if (selectionDirection === 'down') {
			idSelection.reverse();
		}
		idSelection.addMany(
			updatedSelection.map((c) => c.path),
			commitId
		);
	} else {
		// if only one file is selected and it is already selected, unselect it
		if (idSelection.length === 1 && isAlreadySelected) {
			idSelection.clear();
		} else {
			idSelection.set(change.path, commitId);
		}
	}
}
