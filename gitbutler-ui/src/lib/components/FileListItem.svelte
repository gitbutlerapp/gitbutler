<script lang="ts">
	import FileContextMenu from './FileContextMenu.svelte';
	import FileStatusIcons from './FileStatusIcons.svelte';
	import Checkbox from '$lib/components/Checkbox.svelte';
	import { draggable } from '$lib/dragging/draggable';
	import { draggableFile } from '$lib/dragging/draggables';
	import { getVSIFileIcon } from '$lib/ext-icons';
	import { maybeGetContextStore } from '$lib/utils/context';
	import { updateFocus } from '$lib/utils/selection';
	import { Ownership } from '$lib/vbranches/ownership';
	import { Branch, type AnyFile } from '$lib/vbranches/types';
	import { onDestroy } from 'svelte';
	import type { Writable } from 'svelte/store';

	export let file: AnyFile;
	export let isUnapplied: boolean;
	export let selected: boolean;
	export let showCheckbox: boolean = false;
	export let selectedFiles: Writable<AnyFile[]>;
	export let readonly = false;

	const branch = maybeGetContextStore(Branch);
	const selectedOwnership: Writable<Ownership> | undefined = maybeGetContextStore(Ownership);

	let checked = false;
	let indeterminate = false;
	let draggableElt: HTMLDivElement;

	$: if (file && $selectedOwnership) {
		const fileId = file.id;
		checked = file.hunks.every((hunk) => $selectedOwnership?.containsHunk(fileId, hunk.id));
		const selectedCount = file.hunks.filter((hunk) =>
			$selectedOwnership?.containsHunk(fileId, hunk.id)
		).length;
		indeterminate = selectedCount > 0 && file.hunks.length - selectedCount > 0;
	}

	function updateContextMenu() {
		if (popupMenu) popupMenu.$destroy();
		return new FileContextMenu({
			target: document.body
		});
	}

	$: if ($selectedFiles && draggableElt) updateFocus(draggableElt, file, selectedFiles);

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
					if (e.detail) file.hunks.forEach((h) => ownership.addHunk(file.id, h.id));
					if (!e.detail) file.hunks.forEach((h) => ownership.removeHunk(file.id, h.id));
					return ownership;
				});
			}}
		/>
	{/if}
	<div
		bind:this={draggableElt}
		class="file-list-item"
		id={`file-${file.id}`}
		class:selected-draggable={selected}
		on:click
		on:keydown
		on:dragstart={() => {
			// Reset selection if the file being dragged is not in the selected list
			if ($selectedFiles.length > 0 && !$selectedFiles.find((f) => f.id == file.id)) {
				$selectedFiles = [file];
			}
		}}
		use:draggable={{
			...draggableFile($branch?.id || '', file, selectedFiles),
			disabled: readonly || isUnapplied,
			selector: '.selected-draggable'
		}}
		role="button"
		tabindex="0"
		on:contextmenu|preventDefault={(e) =>
			popupMenu.openByMouse(e, {
				files: $selectedFiles.includes(file) ? $selectedFiles : [file]
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
		max-width: 100%;
		overflow: hidden;
		text-align: left;
		user-select: none;
		outline: none;
		margin-bottom: var(--size-2);
		transition: background-color var(--transition-fast);
		background: var(--clr-theme-container-light);
		&:not(.selected-draggable):hover {
			transition: none;
			background-color: color-mix(
				in srgb,
				var(--clr-theme-container-light),
				var(--darken-tint-light)
			);
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
		color: var(--clr-theme-scale-ntrl-0);
		white-space: nowrap;
		flex-shrink: 0;
		text-overflow: ellipsis;
		overflow: hidden;
		line-height: 120%;
	}
	.path {
		color: var(--clr-theme-scale-ntrl-0);
		line-height: 120%;
		flex-shrink: 1;
		white-space: nowrap;
		text-overflow: ellipsis;
		overflow: hidden;
		opacity: 0.3;
	}
	.selected-draggable {
		background-color: var(--clr-theme-scale-pop-80);

		&:hover {
			background-color: color-mix(in srgb, var(--clr-theme-scale-pop-80), var(--darken-extralight));
		}
	}
</style>
