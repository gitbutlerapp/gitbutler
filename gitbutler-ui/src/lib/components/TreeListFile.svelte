<script lang="ts">
	import FileContextMenu from './FileContextMenu.svelte';
	import FileStatusIcons from './FileStatusIcons.svelte';
	import Checkbox from '$lib/components/Checkbox.svelte';
	import { draggable } from '$lib/dragging/draggable';
	import { draggableFile } from '$lib/dragging/draggables';
	import { getVSIFileIcon } from '$lib/ext-icons';
	import { updateFocus } from '$lib/utils/selection';
	import { onDestroy } from 'svelte';
	import type { Project } from '$lib/backend/projects';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { AnyFile } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let project: Project | undefined;
	export let branchId: string;
	export let file: AnyFile;
	export let selected: boolean;
	export let isUnapplied: boolean;
	export let showCheckbox: boolean = false;
	export let selectedOwnership: Writable<Ownership>;
	export let selectedFiles: Writable<AnyFile[]>;
	export let readonly = false;
	export let branchController: BranchController;

	let checked = false;
	let indeterminate = false;
	let draggableElt: HTMLDivElement;

	$: updateOwnership($selectedOwnership);

	function updateOwnership(ownership: Ownership) {
		const fileId = file.id;
		checked = file.hunks.every((hunk) => ownership.containsHunk(fileId, hunk.id));
		const selectedCount = file.hunks.filter((hunk) =>
			ownership.containsHunk(fileId, hunk.id)
		).length;
		indeterminate = selectedCount > 0 && file.hunks.length - selectedCount > 0;
		if (indeterminate) checked = false;
	}

	function updateContextMenu() {
		if (popupMenu) popupMenu.$destroy();
		return new FileContextMenu({
			target: document.body,
			props: { branchController, project }
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
		...draggableFile(branchId, file, selectedFiles),
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
						selectedOwnership.update((ownership) => {
							if (e.detail) file.hunks.forEach((h) => ownership.addHunk(file.id, h.id));
							if (!e.detail) file.hunks.forEach((h) => ownership.removeHunk(file.id, h.id));
							return ownership;
						});
					}}
				/>
			{/if}
			<div class="name-wrapper">
				<img src={getVSIFileIcon(file.path)} alt="js" style="width: var(--space-12)" class="icon" />
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
		margin-bottom: var(--space-2);
		&:last-child {
			margin-bottom: 0;
		}
	}
	.tree-list-file {
		display: inline-flex;
		align-items: center;
		height: var(--size-btn-m);
		padding: var(--space-6) var(--space-8) var(--space-6) var(--space-6);
		gap: var(--space-6);
		border-radius: var(--radius-s);
		width: 100%;
		max-width: 100%;
		outline: none;
		background: var(--clr-theme-container-light);
		&:not(.selected):hover {
			background-color: color-mix(
				in srgb,
				var(--clr-theme-container-light),
				var(--darken-tint-light)
			);
		}
		overflow: hidden;
	}
	.content-wrapper {
		display: flex;
		align-items: center;
		gap: var(--space-10);
		overflow: hidden;
	}
	.name-wrapper {
		display: flex;
		align-items: center;
		gap: var(--space-6);
		overflow: hidden;
	}
	.name {
		color: var(--clr-theme-scale-ntrl-0);
		text-overflow: ellipsis;
		overflow: hidden;
		white-space: nowrap;
	}
	.selected {
		background-color: var(--clr-theme-scale-pop-80);

		&:hover {
			background-color: color-mix(in srgb, var(--clr-theme-scale-pop-80), var(--darken-extralight));
		}
	}
</style>
