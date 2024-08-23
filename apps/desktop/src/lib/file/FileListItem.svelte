<script lang="ts">
	import FileContextMenu from './FileContextMenu.svelte';
	import { draggableChips } from '$lib/dragging/draggable';
	import { DraggableFile } from '$lib/dragging/draggables';
	import { getContext, maybeGetContextStore } from '$lib/utils/context';
	import { computeFileStatus } from '$lib/utils/fileStatus';
	import { getCommitStore } from '$lib/vbranches/contexts';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { Ownership } from '$lib/vbranches/ownership';
	import { VirtualBranch, type AnyFile } from '$lib/vbranches/types';
	import FileListItem from '@gitbutler/ui/file/FileListItem.svelte';
	import type { Writable } from 'svelte/store';

	interface Props {
		file: AnyFile;
		isUnapplied: boolean;
		selected: boolean;
		showCheckbox: boolean;
		readonly: boolean;
		onclick: (e: MouseEvent) => void;
		onkeydown: (e: KeyboardEvent) => void;
	}

	const { file, isUnapplied, selected, showCheckbox, readonly, onclick, onkeydown }: Props =
		$props();

	const branch = maybeGetContextStore(VirtualBranch);
	const selectedOwnership: Writable<Ownership> | undefined = maybeGetContextStore(Ownership);
	const fileIdSelection = getContext(FileIdSelection);
	const commit = getCommitStore();

	const selectedFiles = fileIdSelection.files;

	let contextMenu: FileContextMenu;
	let lastCheckboxDetail = true;

	let draggableEl: HTMLDivElement | undefined = $state();
	let checked = $state(false);

	const draggable = !readonly && !isUnapplied;

	$effect(() => {
		if (!lastCheckboxDetail) {
			selectedOwnership?.update((ownership) => {
				file.hunks.forEach((h) => ownership.remove(file.id, h.id));
				return ownership;
			});
		}
	});

	$effect(() => {
		if (file && $selectedOwnership) {
			checked =
				file.hunks.every((hunk) => $selectedOwnership?.contains(file.id, hunk.id)) &&
				lastCheckboxDetail;
		}
	});

	$effect(() => {
		if (draggableEl) {
			draggableChips(draggableEl, {
				label: `${file.filename}`,
				filePath: file.path,
				data: $selectedFiles.then(
					(files) => new DraggableFile($branch?.id || '', file, $commit, files)
				),
				disabled: !draggable,
				viewportId: 'board-viewport',
				selector: '.selected-draggable'
			});
		}
	});
</script>

<FileContextMenu bind:this={contextMenu} target={draggableEl} {isUnapplied} />

<FileListItem
	bind:ref={draggableEl}
	fileName={file.filename}
	filePath={file.path}
	fileStatus={computeFileStatus(file)}
	{selected}
	{showCheckbox}
	{checked}
	{draggable}
	{onclick}
	{onkeydown}
	oncheck={(e) => {
		const isChecked = e.currentTarget.checked;
		lastCheckboxDetail = isChecked;
		selectedOwnership?.update((ownership) => {
			if (isChecked) {
				file.hunks.forEach((h) => ownership.add(file.id, h));
			} else {
				file.hunks.forEach((h) => ownership.remove(file.id, h.id));
			}
			return ownership;
		});

		$selectedFiles.then((files) => {
			if (files.length > 0 && files.includes(file)) {
				if (isChecked) {
					files.forEach((f) => {
						selectedOwnership?.update((ownership) => {
							f.hunks.forEach((h) => ownership.add(f.id, h));
							return ownership;
						});
					});
				} else {
					files.forEach((f) => {
						selectedOwnership?.update((ownership) => {
							f.hunks.forEach((h) => ownership.remove(f.id, h.id));
							return ownership;
						});
					});
				}
			}
		});
	}}
	ondragstart={async () => {
		// Reset selection if the file being dragged is not in the selected list
		if ($fileIdSelection.length > 0 && !fileIdSelection.has(file.id, $commit?.id)) {
			fileIdSelection.clear();
			fileIdSelection.add(file.id, $commit?.id);
		}

		const files = await $selectedFiles;

		if (files.length > 0) {
			files.forEach((f) => {
				if (f.locked) {
					const lockedElement = document.getElementById(`file-${f.id}`);

					if (lockedElement) {
						// add a class to the locked file
						lockedElement.classList.add('locked-file-animation');
					}
				}
			});
		} else if (file.locked) {
			draggableEl?.classList.add('locked-file-animation');
		}
	}}
	oncontextmenu={async (e) => {
		if (fileIdSelection.has(file.id, $commit?.id)) {
			contextMenu.open(e, { files: await $selectedFiles });
		} else {
			contextMenu.open(e, { files: [file] });
		}
	}}
/>
