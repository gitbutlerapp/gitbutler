/**
 * Shared helper functions for manipulating selected files with keyboard.
 *
 * This file replaces `$lib/utils/selection.ts`, with the main difference
 * being the type change from `AnyFile` to `TreeChange`.
 */
import { type SelectedFile, type SelectionId } from "$lib/selection/key";
import { getSelectionDirection } from "$lib/utils/getSelectionDirection";
import { get } from "svelte/store";
import type { FileSelectionManager } from "$lib/selection/fileSelectionManager.svelte";
import type { TreeChange } from "@gitbutler/but-sdk";

function getFile(
	files: TreeChange[],
	id: string,
	filePathIndices: Map<string, number>,
): TreeChange | undefined {
	const fileIndex = filePathIndices.get(id);
	if (fileIndex === undefined) return undefined;
	return files[fileIndex];
}

function getNextFile(
	files: TreeChange[],
	currentId: string,
	filePathIndices: Map<string, number>,
): TreeChange | undefined {
	const fileIndex = filePathIndices.get(currentId);
	if (fileIndex === undefined) return undefined;

	const nextFileIndex = fileIndex + 1;
	return files[nextFileIndex];
}

function getPreviousFile(
	files: TreeChange[],
	currentId: string,
	filePathIndices: Map<string, number>,
): TreeChange | undefined {
	const fileIndex = filePathIndices.get(currentId);
	if (fileIndex === undefined) return undefined;
	const previousFileIndex = fileIndex - 1;
	return files[previousFileIndex];
}

function getTopFile(files: TreeChange[], selectedPaths: Set<string>): TreeChange | undefined {
	for (const file of files) {
		if (selectedPaths.has(file.path)) {
			return file;
		}
	}
	return undefined;
}

function getBottomFile(files: TreeChange[], selectedPaths: Set<string>): TreeChange | undefined {
	for (let i = files.length - 1; i >= 0; i--) {
		const file = files[i]!;
		if (selectedPaths.has(file.path)) {
			return file;
		}
	}
	return undefined;
}

interface UpdateSelectionParams {
	allowMultiple: boolean;
	ctrlKey: boolean;
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
	ctrlKey,
	metaKey,
	shiftKey,
	key,
	targetElement,
	files,
	selectedFileIds,
	fileIdSelection,
	selectionId,
	preventDefault,
}: UpdateSelectionParams): boolean {
	if (!selectedFileIds[0] || selectedFileIds.length === 0) return false;
	const selectedPaths = new Set(selectedFileIds.map((file) => file.path));
	const filePathIndices = new Map(files.map((file, index) => [file.path, index]));

	const firstFileId = selectedFileIds[0].path;
	const lastFileId = selectedFileIds.at(-1)!.path;

	const topFileId = getTopFile(files, selectedPaths)?.path;
	const bottomFileId = getBottomFile(files, selectedPaths)?.path;

	let selectionDirection = getSelectionDirection(
		filePathIndices.get(lastFileId) ?? -1,
		filePathIndices.get(firstFileId) ?? -1,
	);

	function getAndAddFile(
		id: string,
		getFileFunc?: (
			files: TreeChange[],
			id: string,
			filePathIndices: Map<string, number>,
		) => TreeChange | undefined,
	) {
		const file = getFileFunc?.(files, id, filePathIndices) ?? getFile(files, id, filePathIndices);
		if (file) {
			const fileIndex = filePathIndices.get(file.path);
			if (fileIndex === undefined) return; // should never happen
			fileIdSelection.add(file.path, selectionId, fileIndex);
		}
	}

	function getAndClearExcept(
		id: string,
		getFileFunc?: (
			files: TreeChange[],
			id: string,
			filePathIndices: Map<string, number>,
		) => TreeChange | undefined,
	) {
		const file = getFileFunc?.(files, id, filePathIndices) ?? getFile(files, id, filePathIndices);
		if (file) {
			const fileIndex = filePathIndices.get(file.path);
			if (fileIndex === undefined) return; // should never happen
			fileIdSelection.set(file.path, selectionId, fileIndex);
		}
	}

	switch (key) {
		// Cmd+A on Mac, Ctrl+A on Windows/Linux
		case "a":
		case "A":
			if (allowMultiple && (metaKey || ctrlKey)) {
				preventDefault();

				for (let i = 0; i < files.length; i++) {
					const file = files[i]!;
					fileIdSelection.add(file.path, selectionId, i);
				}

				// Clear file preview after selecting all files (don't show preview for bulk selection)
				fileIdSelection.clearPreview(selectionId);
			}
			break;
		case "k":
		case "ArrowUp":
			preventDefault();
			if (shiftKey && allowMultiple) {
				// Handle case if only one file is selected
				// we should update the selection direction
				if (selectedFileIds.length === 1) {
					selectionDirection = "up";
				} else if (selectionDirection === "down") {
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

		case "j":
		case "ArrowDown":
			preventDefault();
			if (shiftKey && allowMultiple) {
				// Handle case if only one file is selected
				// we should update the selection direction
				if (selectedFileIds.length === 1) {
					selectionDirection = "down";
				} else if (selectionDirection === "up") {
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
		case "Escape":
			preventDefault();
			fileIdSelection.clearPreview(selectionId);
			targetElement.blur();
			return false;
		default:
			return false;
	}
	return true;
}

export function selectFilesInList(
	e: MouseEvent | KeyboardEvent,
	change: TreeChange,
	sortedFiles: TreeChange[],
	idSelection: FileSelectionManager,
	allowMultiple: boolean,
	index: number,
	selectionId: SelectionId,
	allowUnselect?: boolean,
) {
	const isAlreadySelected = idSelection.has(change.path, selectionId);
	const isTheOnlyOneSelected = idSelection.collectionSize(selectionId) === 1 && isAlreadySelected;
	const lastAdded = get(idSelection.getById(selectionId).lastAdded);

	if (e.ctrlKey || e.metaKey) {
		if (isAlreadySelected) {
			idSelection.remove(change.path, selectionId);
			const remainingSelection = idSelection.values(selectionId);
			const previous = remainingSelection.at(-1);
			if (previous) {
				const previousIndex = sortedFiles.findIndex((file) => file.path === previous.path);
				if (previousIndex !== -1) {
					idSelection.add(previous.path, selectionId, previousIndex);
				}
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
