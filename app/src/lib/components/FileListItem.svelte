<script lang="ts">
	import FileContextMenu from './FileContextMenu.svelte';
	import FileStatusIcons from './FileStatusIcons.svelte';
	import Checkbox from '$lib/components/Checkbox.svelte';
	import { draggable } from '$lib/dragging/draggable';
	import { DraggableFile } from '$lib/dragging/draggables';
	import { getVSIFileIcon } from '$lib/ext-icons';
	import { getContext, maybeGetContextStore } from '$lib/utils/context';
	import { updateFocus } from '$lib/utils/selection';
	import { getCommitStore, getSelectedFiles } from '$lib/vbranches/contexts';
	import { FileSelection, fileKey } from '$lib/vbranches/fileSelection';
	import { Ownership } from '$lib/vbranches/ownership';
	import { Branch, type AnyFile } from '$lib/vbranches/types';
	import { onDestroy } from 'svelte';
	import type { Writable } from 'svelte/store';

	export let file: AnyFile;
	export let isUnapplied: boolean;
	export let selected: boolean;
	export let showCheckbox: boolean = false;
	export let readonly = false;

	const branch = maybeGetContextStore(Branch);
	const selectedOwnership: Writable<Ownership> | undefined = maybeGetContextStore(Ownership);
	const fileSelection = getContext(FileSelection);
	const selectedFiles = getSelectedFiles();
	const commit = getCommitStore();

	let checked = false;
	let indeterminate = false;
	let draggableElt: HTMLDivElement;

	$: if (file && $selectedOwnership) {
		const fileId = file.id;
		checked = file.hunks.every((hunk) => $selectedOwnership?.contains(fileId, hunk.id));
		const selectedCount = file.hunks.filter((hunk) =>
			$selectedOwnership?.contains(fileId, hunk.id)
		).length;
		indeterminate = selectedCount > 0 && file.hunks.length - selectedCount > 0;
	}

	function updateContextMenu() {
		if (popupMenu) popupMenu.$destroy();
		return new FileContextMenu({
			target: document.body
		});
	}

	$: if ($fileSelection && draggableElt)
		updateFocus(draggableElt, file, fileSelection, $commit?.id);

	$: popupMenu = updateContextMenu();

	onDestroy(() => {
		if (popupMenu) {
			popupMenu.$destroy();
		}
	});
</script>

<div class="list-item-wrapper">
	{#if showCheckbox}
		<Checkbox
			small
			{checked}
			{indeterminate}
			on:change={(e) => {
				selectedOwnership?.update((ownership) => {
					if (e.detail) file.hunks.forEach((h) => ownership.add(file.id, h));
					if (!e.detail) file.hunks.forEach((h) => ownership.remove(file.id, h.id));
					return ownership;
				});
			}}
		/>
	{/if}
	<div
		bind:this={draggableElt}
		class="file-list-item"
		class:selected-draggable={selected}
		id={`file-${file.id}`}
		data-locked={file.locked}
		on:click
		on:keydown
		on:dragstart={() => {
			// Reset selection if the file being dragged is not in the selected list
			if ($fileSelection.length > 0 && !fileSelection.has(file.id, $commit?.id)) {
				fileSelection.clear();
				fileSelection.add(file.id, $commit?.id);
			}

			if ($selectedFiles.length > 0) {
				$selectedFiles.forEach((f) => {
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
		use:draggable={{
			data: new DraggableFile($branch?.id || '', file, selectedFiles),
			disabled: readonly || isUnapplied,
			viewportId: 'board-viewport',
			selector: '.selected-draggable'
		}}
		role="button"
		tabindex="0"
		on:contextmenu|preventDefault={(e) =>
			popupMenu.openByMouse(e, {
				files: fileSelection.has(file.id, $commit?.id)
					? $fileSelection.map((key) =>
							$branch?.files.find((f) => fileKey(f.id, $commit?.id) == key)
						)
					: [file]
			})}
	>
		<div class="info-wrap">
			<div class="info">
				<img draggable="false" class="file-icon" src={getVSIFileIcon(file.path)} alt="js" />
				<span class="text-base-12 name">
					{file.filename}
				</span>
				<span class="text-base-12 path">
					{file.justpath}
				</span>
			</div>
		</div>
		<FileStatusIcons {file} />
	</div>
</div>

<style lang="postcss">
	.list-item-wrapper {
		display: flex;
		align-items: center;
		gap: var(--size-8);
	}

	.file-list-item {
		flex: 1;
		display: flex;
		align-items: center;
		height: var(--size-28);
		padding: var(--size-4) var(--size-8);
		gap: var(--size-16);
		border-radius: var(--radius-s);
		overflow: hidden;
		text-align: left;
		user-select: none;
		outline: none;
		background: var(--clr-bg-main);
		border: 1px solid transparent;

		&:not(.selected-draggable):hover {
			background-color: var(--clr-bg-muted);
		}
	}

	.info-wrap {
		display: flex;
		align-items: center;
		flex-grow: 1;
		flex-shrink: 1;
		gap: var(--size-10);
		overflow: hidden;
	}
	.info {
		display: flex;
		align-items: center;
		flex-grow: 1;
		flex-shrink: 1;
		gap: var(--size-6);
		overflow: hidden;
	}

	.file-icon {
		width: var(--size-12);
	}
	.name {
		color: var(--clr-scale-ntrl-0);
		white-space: nowrap;
		flex-shrink: 0;
		text-overflow: ellipsis;
		overflow: hidden;
		line-height: 120%;
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

	.selected-draggable {
		background-color: var(--clr-scale-pop-80);
		border: 1px solid var(--clr-bg-main);

		&:hover {
			background-color: var(--clr-scale-pop-80);
		}
	}
</style>
