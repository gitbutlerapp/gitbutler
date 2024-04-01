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
	export let selected: boolean;
	export let isUnapplied: boolean;
	export let showCheckbox: boolean = false;
	export let selectedFiles: Writable<AnyFile[]>;
	export let readonly = false;

	let checked = false;
	let indeterminate = false;
	let draggableElt: HTMLDivElement;

	const selectedOwnership: Writable<Ownership> | undefined = maybeGetContextStore(Ownership);
	const branch = maybeGetContextStore(Branch);

	$: updateOwnership($selectedOwnership);

	function updateOwnership(ownership: Ownership | undefined) {
		if (!ownership) return;
		const fileId = file.id;
		checked = file.hunks.every((hunk) => ownership.contains(fileId, hunk.id));
		const selectedCount = file.hunks.filter((hunk) => ownership.contains(fileId, hunk.id)).length;
		indeterminate = selectedCount > 0 && file.hunks.length - selectedCount > 0;
		if (indeterminate) checked = false;
	}

	function updateContextMenu() {
		if (popupMenu) popupMenu.$destroy();
		return new FileContextMenu({
			target: document.body
		});
	}

	$: popupMenu = updateContextMenu();

	$: if ($selectedFiles && draggableElt) updateFocus(draggableElt, file, selectedFiles);

	onDestroy(() => {
		if (popupMenu) {
			popupMenu.$destroy();
		}
	});
</script>

<div
	bind:this={draggableElt}
	on:dragstart={() => {
		// Reset selection if the file being dragged is not in the selected list
		if ($selectedFiles.length > 0 && !$selectedFiles.find((f) => f.id == file.id)) {
			$selectedFiles = [];
		}
	}}
	use:draggable={{
		...draggableFile($branch?.id || '', file, selectedFiles),
		disabled: readonly || isUnapplied,
		selector: '.selected-draggable'
	}}
	on:click
	on:keydown
	on:contextmenu|preventDefault={(e) =>
		popupMenu.openByMouse(e, {
			files: $selectedFiles.includes(file) ? $selectedFiles : [file]
		})}
	class="draggable-wrapper"
	role="button"
	tabindex="0"
>
	<div class="tree-list-file" class:selected role="listitem" on:contextmenu|preventDefault>
		<div class="content-wrapper">
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
			<div class="name-wrapper">
				<img src={getVSIFileIcon(file.path)} alt="js" style="width: var(--size-12)" class="icon" />
				<span class="name text-base-12">
					{file.filename}
				</span>
				<FileStatusIcons {file} />
			</div>
		</div>
	</div>
</div>

<style lang="postcss">
	.draggable-wrapper {
		display: inline-block;
		width: 100%;
		margin-bottom: var(--size-2);
		&:last-child {
			margin-bottom: 0;
		}
	}
	.tree-list-file {
		display: inline-flex;
		align-items: center;
		height: var(--size-control-button);
		padding: var(--size-6) var(--size-8) var(--size-6) var(--size-6);
		gap: var(--size-6);
		border-radius: var(--radius-s);
		width: 100%;
		max-width: 100%;
		outline: none;
		background: var(--clr-container-light);
		&:not(.selected):hover {
			background-color: color-mix(in srgb, var(--clr-container-light), var(--darken-tint-light));
		}
		overflow: hidden;
	}
	.content-wrapper {
		display: flex;
		align-items: center;
		gap: var(--size-10);
		overflow: hidden;
	}
	.name-wrapper {
		display: flex;
		align-items: center;
		gap: var(--size-6);
		overflow: hidden;
	}
	.name {
		color: var(--clr-scale-ntrl-0);
		text-overflow: ellipsis;
		overflow: hidden;
		white-space: nowrap;
	}
	.selected {
		background-color: var(--clr-scale-pop-80);

		&:hover {
			background-color: color-mix(in srgb, var(--clr-scale-pop-80), var(--darken-extralight));
		}
	}
</style>
