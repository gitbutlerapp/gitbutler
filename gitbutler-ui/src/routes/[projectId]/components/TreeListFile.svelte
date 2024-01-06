<script lang="ts">
	import Checkbox from '$lib/components/Checkbox.svelte';
	import { draggableFile } from '$lib/draggables';
	import { getVSIFileIcon } from '$lib/ext-icons';
	import { draggable } from '$lib/utils/draggable';
	import type { File } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';
	import FileStatusIcons from './FileStatusIcons.svelte';
	import type { Ownership } from '$lib/vbranches/ownership';

	export let branchId: string;
	export let file: File;
	export let selected: boolean;
	export let readonly: boolean;
	export let showCheckbox: boolean = false;
	export let selectedOwnership: Writable<Ownership>;
	export let selectedFiles: Writable<File[]>;

	let checked = false;
	let indeterminate = false;

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
</script>

<div
	on:dragstart={() => {
		// Reset selection if the file being dragged is not in the selected list
		if ($selectedFiles.length > 0 && !$selectedFiles.find((f) => f.id == file.id)) {
			$selectedFiles = [];
		}
	}}
	use:draggable={{
		...draggableFile(branchId, file, selectedFiles),
		disabled: readonly,
		selector: '.selected'
	}}
	on:click
	on:keydown
	class="draggable-wrapper"
	role="button"
	tabindex="0"
>
	<div class="tree-list-file" class:selected>
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
		max-width: 100%;
		background: var(--clr-theme-container-light);
		&:not(.selected):hover {
			background: var(--clr-theme-container-pale);
		}
		overflow: hidden;
	}
	.content-wrapper {
		display: flex;
		align-items: center;
		gap: var(--space-10);
	}
	.name-wrapper {
		display: flex;
		align-items: center;
		gap: var(--space-6);
	}
	.name {
		color: var(--clr-theme-scale-ntrl-0);
		text-overflow: ellipsis;
		overflow: hidden;
	}
	.selected {
		background-color: var(--clr-theme-scale-pop-80);
	}
</style>
