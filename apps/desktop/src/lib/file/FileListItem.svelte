<script lang="ts">
	import FileContextMenu from './FileContextMenu.svelte';
	import { draggableChips } from '$lib/dragging/draggable';
	import { DraggableFile } from '$lib/dragging/draggables';
	import { itemsSatisfy } from '$lib/utils/array';
	import { getContext, maybeGetContextStore } from '$lib/utils/context';
	import { computeFileStatus } from '$lib/utils/fileStatus';
	import { getLocalCommits, getLocalAndRemoteCommits } from '$lib/vbranches/contexts';
	import { getCommitStore } from '$lib/vbranches/contexts';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { SelectedOwnership } from '$lib/vbranches/ownership';
	import { getLockText } from '$lib/vbranches/tooltip';
	import { VirtualBranch, type AnyFile, LocalFile } from '$lib/vbranches/types';
	import FileListItem from '@gitbutler/ui/file/FileListItem.svelte';
	import { onDestroy } from 'svelte';
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
	const selectedOwnership: Writable<SelectedOwnership> | undefined =
		maybeGetContextStore(SelectedOwnership);
	const fileIdSelection = getContext(FileIdSelection);
	const commit = getCommitStore();

	// TODO: Refactor this into something more meaningful.
	const localCommits = file instanceof LocalFile ? getLocalCommits() : undefined;
	const remoteCommits = file instanceof LocalFile ? getLocalAndRemoteCommits() : undefined;
	let lockedIds = file.lockedIds;
	let lockText = $derived(
		lockedIds.length > 0 && $localCommits
			? getLockText(lockedIds, ($localCommits || []).concat($remoteCommits || []))
			: ''
	);

	const selectedFiles = fileIdSelection.files;

	let contextMenu: FileContextMenu;

	let draggableEl: HTMLDivElement | undefined = $state();
	let checked = $state(false);
	let indeterminate = $state(false);

	const draggable = !readonly && !isUnapplied;

	$effect(() => {
		if (file && $selectedOwnership) {
			const hunksContained = itemsSatisfy(file.hunks, (h) =>
				$selectedOwnership?.isSelected(file.id, h.id)
			);
			checked = hunksContained === 'all';
			indeterminate = hunksContained === 'some';
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

<FileContextMenu
	bind:this={contextMenu}
	target={draggableEl}
	{isUnapplied}
	branchId={$branch?.id}
/>

<FileListItem
	id={`file-${file.id}`}
	bind:ref={draggableEl}
	fileName={file.filename}
	filePath={file.path}
	fileStatus={computeFileStatus(file)}
	{selected}
	{showCheckbox}
	{checked}
	{indeterminate}
	{draggable}
	{onclick}
	{onkeydown}
	locked={file.locked}
	{lockText}
	oncheck={(e) => {
		const isChecked = e.currentTarget.checked;
		selectedOwnership?.update((ownership) => {
			if (isChecked) {
				file.hunks.forEach((h) => ownership.select(file.id, h));
			} else {
				file.hunks.forEach((h) => ownership.ignore(file.id, h.id));
			}
			return ownership;
		});

		$selectedFiles.then((files) => {
			if (files.length > 0 && files.includes(file)) {
				if (isChecked) {
					files.forEach((f) => {
						selectedOwnership?.update((ownership) => {
							f.hunks.forEach((h) => ownership.select(f.id, h));
							return ownership;
						});
					});
				} else {
					files.forEach((f) => {
						selectedOwnership?.update((ownership) => {
							f.hunks.forEach((h) => ownership.ignore(f.id, h.id));
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
		let animationEndHandler: () => void;

		function addAnimationEndListener(element: HTMLElement) {
			animationEndHandler = () => {
				element.classList.remove('locked-file-animation');
				element.removeEventListener('animationend', animationEndHandler);
			};
			element.addEventListener('animationend', animationEndHandler);
		};

		if (files.length > 0) {
			files.forEach((f) => {
				if (f.locked) {
					const lockedElement = document.getElementById(`file-${f.id}`);
					if (lockedElement) {
						lockedElement.classList.add('locked-file-animation');
						addAnimationEndListener(lockedElement);
					}
				}
			});
		} else if (file.locked) {
			draggableEl?.classList.add('locked-file-animation');
			draggableEl && addAnimationEndListener(draggableEl);
		}

		onDestroy(() => {
			// Ensure any listeners are removed if the component is destroyed before animation ends
			if (draggableEl && animationEndHandler) {
				draggableEl.removeEventListener('animationend', animationEndHandler);
			}
			files.forEach((f) => {
				const lockedElement = document.getElementById(`file-${f.id}`);
				if (lockedElement && animationEndHandler) {
					lockedElement.removeEventListener('animationend', animationEndHandler);
				}
			});
		});
	}}
	oncontextmenu={async (e) => {
		if (fileIdSelection.has(file.id, $commit?.id)) {
			contextMenu.open(e, { files: await $selectedFiles });
		} else {
			contextMenu.open(e, { files: [file] });
		}
	}}
/>
