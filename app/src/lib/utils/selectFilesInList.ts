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
			(file) => stringifyFileKey(file.id, undefined) == selectedFileIds[0]
		);

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

		selectedFileIds = updatedSelection.map((f) => stringifyFileKey(f.id, commit?.id));

		if (selectionDirection === 'down') {
			selectedFileIds = selectedFileIds.reverse();
		}
		fileIdSelection.set(selectedFileIds);
	} else {
		// if only one file is selected and it is already selected, unselect it
		if (selectedFileIds.length == 1 && isAlreadySelected) {
			fileIdSelection.clear();
		} else {
			fileIdSelection.set([stringifyFileKey(file.id, commit?.id)]);
		}
	}
}
