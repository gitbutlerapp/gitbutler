<script lang="ts">
	import FileContextMenu from './FileContextMenu.svelte';
	import { draggableChips, type DraggableConfig } from '$lib/dragging/draggable';
	import { DraggableFile } from '$lib/dragging/draggables';
	import { itemsSatisfy } from '$lib/utils/array';
	import { computeFileStatus } from '$lib/utils/fileStatus';
	import { getLocalCommits, getLocalAndRemoteCommits } from '$lib/vbranches/contexts';
	import { getCommitStore } from '$lib/vbranches/contexts';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { SelectedOwnership } from '$lib/vbranches/ownership';
	import { getLockText } from '$lib/vbranches/tooltip';
	import { VirtualBranch, type AnyFile, LocalFile } from '$lib/vbranches/types';
	import { getContext, maybeGetContextStore } from '@gitbutler/shared/context';
	import FileListItem from '@gitbutler/ui/file/FileListItem.svelte';
	import type { Writable } from 'svelte/store';

	interface Props {
		file: AnyFile;
		isUnapplied: boolean;
		selected: boolean;
		showCheckbox: boolean;
		readonly: boolean;
		onclick: (e: MouseEvent) => void;
		onkeydown?: (e: KeyboardEvent) => void;
	}

	const { file, isUnapplied, selected, showCheckbox, readonly, onclick, onkeydown }: Props =
		$props();

	const branch = maybeGetContextStore(VirtualBranch);
	const branchId = $derived($branch?.id);
	const selectedOwnership: Writable<SelectedOwnership> | undefined =
		maybeGetContextStore(SelectedOwnership);
	const fileIdSelection = getContext(FileIdSelection);
	const commit = getCommitStore();

	// TODO: Refactor this into something more meaningful.
	const localCommits = file instanceof LocalFile ? getLocalCommits() : undefined;
	const remoteCommits = file instanceof LocalFile ? getLocalAndRemoteCommits() : undefined;
	let lockedIds = file.lockedIds;
	const lockText = $derived(
		lockedIds.length > 0 && $localCommits
			? getLockText(lockedIds, ($localCommits || []).concat($remoteCommits || []))
			: ''
	);

	const selectedFiles = fileIdSelection.files;
	const draggable = !readonly && !isUnapplied;

	let contextMenu = $state<ReturnType<typeof FileContextMenu>>();
	let draggableEl: HTMLDivElement | undefined = $state();
	let indeterminate = $state(false);
	let checked = $state(false);

	$effect(() => {
		if (file && $selectedOwnership) {
			const hunksContained = itemsSatisfy(file.hunks, (h) =>
				$selectedOwnership?.isSelected(file.id, h.id)
			);
			checked = hunksContained === 'all';
			indeterminate = hunksContained === 'some';
		}
	});

	// TODO: Refactor to use this as a Svelte action, e.g. `use:draggableChips()`.
	let chips:
		| {
				update: (opts: DraggableConfig) => void;
				destroy: () => void;
		  }
		| undefined;

	// Manage the lifecycle of the draggable chips.
	$effect(() => {
		if (draggableEl) {
			const draggableFile = new DraggableFile(branchId || '', file, $commit, selectedFiles);
			const config = {
				label: `${file.filename}`,
				filePath: file.path,
				dropData: draggableFile,
				disabled: !draggable,
				viewportId: 'board-viewport',
				selector: '.selected-draggable'
			};
			if (chips) {
				chips.update(config);
			} else {
				chips = draggableChips(draggableEl, config);
			}
		} else {
			chips?.destroy();
		}

		return () => {
			chips?.destroy();
		};
	});
</script>

<FileContextMenu
	bind:this={contextMenu}
	trigger={draggableEl}
	{isUnapplied}
	branchId={$branch?.id}
	isBinary={file.binary}
/>

<FileListItem
	id={file.id}
	bind:ref={draggableEl}
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
	conflicted={file.conflicted}
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

		if ($selectedFiles.length > 0 && $selectedFiles.includes(file)) {
			if (isChecked) {
				$selectedFiles.forEach((f) => {
					selectedOwnership?.update((ownership) => {
						f.hunks.forEach((h) => ownership.select(f.id, h));
						return ownership;
					});
				});
			} else {
				$selectedFiles.forEach((f) => {
					selectedOwnership?.update((ownership) => {
						f.hunks.forEach((h) => ownership.ignore(f.id, h.id));
						return ownership;
					});
				});
			}
		}
	}}
	oncontextmenu={(e) => {
		if (fileIdSelection.has(file.id, $commit?.id)) {
			contextMenu?.open(e, { files: $selectedFiles });
		} else {
			contextMenu?.open(e, { files: [file] });
		}
	}}
/>
