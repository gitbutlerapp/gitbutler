<script lang="ts">
	import FileContextMenu from './FileContextMenu.svelte';
	import FileStatusIcons from './FileStatusIcons.svelte';
	import Checkbox from '$lib/components/Checkbox.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import { draggable } from '$lib/dragging/draggable';
	import { DraggableFile } from '$lib/dragging/draggables';
	import { getVSIFileIcon } from '$lib/ext-icons';
	import { getContext, maybeGetContextStore } from '$lib/utils/context';
	import { updateFocus } from '$lib/utils/selection';
	import { getCommitStore } from '$lib/vbranches/contexts';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { Ownership } from '$lib/vbranches/ownership';
	import { Branch, type AnyFile } from '$lib/vbranches/types';
	import { mount, onDestroy, unmount } from 'svelte';
	import type { Writable } from 'svelte/store';

	export let file: AnyFile;
	export let isUnapplied: boolean;
	export let selected: boolean;
	export let showCheckbox: boolean = false;
	export let readonly = false;

	const branch = maybeGetContextStore(Branch);
	const selectedOwnership: Writable<Ownership> | undefined = maybeGetContextStore(Ownership);
	const fileIdSelection = getContext(FileIdSelection);
	const commit = getCommitStore();

	const selectedFiles = fileIdSelection.files;

	let checked = false;
	let draggableElt: HTMLDivElement;

	$: if (file && $selectedOwnership) {
		checked = file.hunks.every((hunk) => $selectedOwnership?.contains(file.id, hunk.id));
	}

	$: if ($fileIdSelection && draggableElt)
		updateFocus(draggableElt, file, fileIdSelection, $commit?.id);

	$: popupMenu = updateContextMenu();

	function updateContextMenu() {
		if (popupMenu) unmount(popupMenu);
		return mount(FileContextMenu, {
			target: document.body,
			props: { isUnapplied }
		});
	}

	onDestroy(() => {
		if (popupMenu) {
			unmount(popupMenu);
		}
	});

	const isDraggable = !readonly && !isUnapplied;
</script>

<div
	bind:this={draggableElt}
	class="file-list-item"
	class:selected-draggable={selected}
	class:draggable={isDraggable}
	id={`file-${file.id}`}
	data-locked={file.locked}
	on:click
	on:keydown
	on:dragstart={async () => {
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
			draggableElt.classList.add('locked-file-animation');
		}
	}}
	on:animationend={() => {
		// remove the class after the animation ends
		if (file.locked) {
			draggableElt.classList.remove('locked-file-animation');
		}
	}}
	role="button"
	tabindex="0"
	on:contextmenu|preventDefault={async (e) => {
		if (fileIdSelection.has(file.id, $commit?.id)) {
			popupMenu.openByMouse(e, { files: await $selectedFiles });
		} else {
			popupMenu.openByMouse(e, { files: [file] });
		}
	}}
	use:draggable={{
		data: $selectedFiles.then(
			(files) => new DraggableFile($branch?.id || '', file, $commit, files)
		),
		disabled: !isDraggable,
		viewportId: 'board-viewport',
		selector: '.selected-draggable'
	}}
>
	{#if showCheckbox}
		<Checkbox
			small
			{checked}
			on:change={(e) => {
				const isChecked = e.detail;
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
		/>
	{/if}
	<div class="info">
		<img draggable="false" class="file-icon" src={getVSIFileIcon(file.path)} alt="" />
		<span class="text-base-12 name">
			{file.filename}
		</span>
		<span class="text-base-12 path">
			{file.justpath}
		</span>
	</div>
	<FileStatusIcons {file} />
	{#if isDraggable}
		<div class="draggable-handle">
			<Icon name="draggable-narrow" />
		</div>
	{/if}
</div>

<style lang="postcss">
	.file-list-item {
		flex: 1;
		display: flex;
		align-items: center;
		padding: 6px 14px;
		gap: 10px;
		height: 32px;
		overflow: hidden;
		text-align: left;
		user-select: none;
		outline: none;
		background: var(--clr-bg-1);
		border-bottom: 1px solid var(--clr-border-3);

		&:not(.selected-draggable):hover {
			background-color: var(--clr-bg-1-muted);
		}

		&:last-child {
			border-bottom: none;
		}
	}

	.draggable {
		&:hover {
			& .draggable-handle {
				opacity: 1;
			}
		}
	}

	.draggable-handle {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 4px;
		color: var(--clr-text-3);
		opacity: 0;
		margin-left: -2px;
		margin-right: -8px;
		transition:
			width var(--transition-fast),
			opacity var(--transition-fast);
	}

	.info {
		display: flex;
		align-items: center;
		flex-grow: 1;
		flex-shrink: 1;
		gap: 6px;
		overflow: hidden;
	}

	.file-icon {
		width: 12px;
	}

	.name {
		color: var(--clr-scale-ntrl-0);
		white-space: nowrap;
		flex-shrink: 0;
		text-overflow: ellipsis;
		overflow: hidden;
	}

	.path {
		color: var(--clr-scale-ntrl-0);
		line-height: 120%;
		flex-shrink: 1;
		white-space: nowrap;
		text-overflow: ellipsis;
		overflow: hidden;
		opacity: 0.3;
	}

	/* MODIFIERS */

	.selected-draggable {
		background-color: var(--clr-theme-pop-bg);
	}
</style>
