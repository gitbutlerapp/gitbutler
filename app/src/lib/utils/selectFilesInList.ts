import { getSelectionDirection } from './getSelectionDirection';
import { stringifyFileKey, type FileIdSelection } from '$lib/vbranches/fileIdSelection';
import { get } from 'svelte/store';
import type { AnyCommit, AnyFile } from '$lib/vbranches/types';

export function selectFilesInList(
	e: MouseEvent,
	file: AnyFile,
	fileIdSelection: FileIdSelection,
	sortedFiles: AnyFile[],
	allowMultiple: boolean,
	commit: AnyCommit | undefined
) {
	let selectedFileIds = get(fileIdSelection);
	e.stopPropagation();
	const isAlreadySelected = selectedFileIds && fileIdSelection.has(file.id, commit?.id);

	if (e.ctrlKey || e.metaKey) {
		if (isAlreadySelected) {
			fileIdSelection.remove(file.id, commit?.id);
		} else {
			fileIdSelection.add(file.id, commit?.id);
		}
	} else if (e.shiftKey && allowMultiple) {
		const initiallySelectedIndex = sortedFiles.findIndex(
			(f) => f.id === fileIdSelection.only()?.fileId
		);

		// detect the direction of the selection
		const selectionDirection = getSelectionDirection(
			initiallySelectedIndex,
			sortedFiles.findIndex((f) => f.id === file.id)
		);

		const updatedSelection = sortedFiles.slice(
			Math.min(
				initiallySelectedIndex,
				sortedFiles.findIndex((f) => f.id === file.id)
			),
			Math.max(
				initiallySelectedIndex,
				sortedFiles.findIndex((f) => f.id === file.id)
			) + 1
		);

		selectedFileIds = updatedSelection.map((f) => stringifyFileKey(f.id, commit?.id));

		// if the selection is in the opposite direction, reverse the selection
		if (selectionDirection === 'down') {
			selectedFileIds = selectedFileIds.reverse();
		}

		fileIdSelection.set(selectedFileIds);
	} else {
		// if only one file is selected and it is already selected, unselect it
		if (selectedFileIds.length === 1 && isAlreadySelected) {
			fileIdSelection.clear();
		} else {
			fileIdSelection.set([stringifyFileKey(file.id, commit?.id)]);
		}
	}
}
