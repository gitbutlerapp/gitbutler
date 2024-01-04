<script lang="ts">
	import Checkbox from '$lib/components/Checkbox.svelte';
	import { draggableFile } from '$lib/draggables';
	import { getVSIFileIcon } from '$lib/ext-icons';
	import { draggable } from '$lib/utils/draggable';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { File } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';
	import FileStatusIcons from './FileStatusIcons.svelte';

	export let branchId: string;
	export let file: File;
	export let readonly: boolean;
	export let selected: boolean;
	export let showCheckbox: boolean = false;
	export let selectedOwnership: Writable<Ownership>;

	let checked = false;
	let indeterminate = false;

	$: if (file) {
		const fileId = file.id;
		checked = file.hunks.every((hunk) => $selectedOwnership.containsHunk(fileId, hunk.id));
		const selectedCount = file.hunks.filter((hunk) =>
			$selectedOwnership.containsHunk(fileId, hunk.id)
		).length;
		indeterminate = selectedCount > 0 && file.hunks.length - selectedCount > 0;
	}
</script>

<div
	on:click
	on:keydown
	use:draggable={{
		...draggableFile(branchId, file),
		disabled: readonly
	}}
	role="button"
	tabindex="0"
>
	<div class="file-list-item" id={`file-${file.id}`} class:selected>
		<div class="info-wrap">
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
			<div class="info">
				<img src={getVSIFileIcon(file.path)} alt="js" style="width: var(--space-12)" />
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
	.file-list-item {
		display: flex;
		align-items: center;
		height: var(--size-btn-m);
		padding: var(--space-4) var(--space-8);
		gap: var(--space-16);
		border-radius: var(--radius-s);
		max-width: 100%;
		overflow: hidden;
		background: var(--clr-theme-container-light);
		text-align: left;
		&:not(.selected):hover {
			background: var(--clr-theme-container-pale);
		}
	}
	.info-wrap {
		display: flex;
		align-items: center;
		flex-grow: 1;
		flex-shrink: 1;
		gap: var(--space-10);
		overflow: hidden;
	}
	.info {
		display: flex;
		align-items: center;
		flex-grow: 1;
		flex-shrink: 1;
		gap: var(--space-6);
		overflow: hidden;
	}
	.name {
		color: var(--clr-theme-scale-ntrl-0);
		white-space: nowrap;
		flex-shrink: 0;
		text-overflow: ellipsis;
		overflow: hidden;
	}
	.path {
		color: var(--clr-theme-scale-ntrl-40);
		line-height: 120%;
		flex-shrink: 1;
		white-space: nowrap;
		text-overflow: ellipsis;
		overflow: hidden;
	}
	.selected {
		background-color: var(--clr-theme-scale-pop-80);
	}
</style>
