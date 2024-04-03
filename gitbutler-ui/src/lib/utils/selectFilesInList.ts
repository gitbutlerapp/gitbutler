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

	if (e.ctrlKey || e.metaKey) {
		if (isAlreadySelected) {
			fileSelection.remove(file.id, commit?.id);
		} else {
			fileSelection.add(file.id, commit?.id);
		}
	} else if (e.shiftKey && allowMultiple) {
		const initiallySelectedIndex = sortedFiles.findIndex(
			(f) => f.id + '|' + undefined == selectedFileIds[0]
		);

		// detect the direction of the selection
		const selectionDirection =
			initiallySelectedIndex < sortedFiles.findIndex((f) => f.id == file.id + '|' + commit?.id)
				? 'down'
				: 'up';

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

		selectedFileIds = updatedSelection.map((f) => f.id + '|' + commit?.id);

		if (selectionDirection === 'down') {
			selectedFileIds = selectedFileIds.reverse();
		}
		fileSelection.set(selectedFileIds);
	} else {
		// if only one file is selected and it is already selected, unselect it
		if (selectedFileIds.length == 1 && isAlreadySelected) {
			fileSelection.clear();
		} else {
			fileSelection.set([file.id + '|' + commit?.id]);
		}
	}
}
