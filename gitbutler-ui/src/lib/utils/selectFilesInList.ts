import { get } from 'svelte/store';
import type { FileSelection } from '$lib/vbranches/fileSelection';
import type { AnyCommit, AnyFile } from '$lib/vbranches/types';

export function selectFilesInList(
	e: MouseEvent,
	file: AnyFile,
	fileSelection: FileSelection,
	sortedFiles: AnyFile[],
	allowMultiple: boolean,
	commit: AnyCommit | undefined
) {
	let selectedFileIds = get(fileSelection.fileIds);
	e.stopPropagation();
	const isAlreadySelected = selectedFileIds && fileSelection.has(file.id, commit?.id);

	// Ctrl + Click or Cmd + Click to select multiple files
	if (e.ctrlKey || e.metaKey) {
		// if file is already selected, unselect it
		if (isAlreadySelected) {
			fileSelection.remove(file.id, commit?.id);
		} else {
			fileSelection.add(file.id, commit?.id);
		}
	}
	// Shift + Click to select range
	else if (e.shiftKey && allowMultiple) {
		const initiallySelectedIndex = sortedFiles.findIndex((f) => f.id == selectedFileIds[0]);

		// detect the direction of the selection
		const selectionDirection =
			initiallySelectedIndex < sortedFiles.findIndex((f) => f.id == file.id) ? 'down' : 'up';

		const updatedSelection = sortedFiles.slice(
			Math.min(
				initiallySelectedIndex,
				sortedFiles.findIndex((f) => f.id == file.id)
			),
			Math.max(
				initiallySelectedIndex,
				sortedFiles.findIndex((f) => f.id == file.id)
			) + 1
		);

		selectedFileIds = updatedSelection.map((f) => f.id);

		if (selectionDirection === 'down') {
			selectedFileIds = selectedFileIds.reverse();
		}
	}
	// select only one file
	else {
		// if only one file is selected and it is already selected, unselect it
		if (selectedFileIds.length == 1 && isAlreadySelected) {
			selectedFileIds = [];
		} else {
			selectedFileIds = [file.id];
		}
	}
	fileSelection.set(selectedFileIds);
}
