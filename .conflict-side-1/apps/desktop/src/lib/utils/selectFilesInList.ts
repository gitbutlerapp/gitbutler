import { getSelectionDirection } from './getSelectionDirection';
import { type FileIdSelection } from '$lib/selection/fileIdSelection';
import { get } from 'svelte/store';
import type { AnyCommit } from '$lib/commits/commit';
import type { AnyFile } from '$lib/files/file';

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
			fileIdSelection.add(file, commit?.id);
		}
	} else if (e.shiftKey && allowMultiple) {
		// TODO(CTO): Not sure that this is accurate.
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

		// if the selection is in the opposite direction, reverse the selection
		if (selectionDirection === 'down') {
			selectedFileIds = selectedFileIds.reverse();
		}
		fileIdSelection.selectMany(updatedSelection, commit?.id);
	} else {
		// if only one file is selected and it is already selected, unselect it
		if (selectedFileIds.length === 1 && isAlreadySelected) {
			fileIdSelection.clear();
		} else {
			fileIdSelection.set(file, commit?.id);
		}
	}
}
