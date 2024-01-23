<script lang="ts">
	import { filesToFileTree } from '$lib/vbranches/filetree';
	import type { Branch, File } from '$lib/vbranches/types';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { Writable } from 'svelte/store';
	import Badge from '$lib/components/Badge.svelte';
	import SegmentedControl from '$lib/components/SegmentControl/SegmentedControl.svelte';
	import Segment from '$lib/components/SegmentControl/Segment.svelte';
	import FileTree from './FileTree.svelte';
	import Checkbox from '$lib/components/Checkbox.svelte';
	import BranchFilesList from './BranchFilesList.svelte';

	export let branch: Branch;
	export let readonly: boolean;
	export let selectedOwnership: Writable<Ownership>;
	export let selectedFiles: Writable<File[]>;
	export let showCheckboxes = false;

	let selectedListMode: string;

	let headerElement: HTMLDivElement;

	function isAllChecked(selectedOwnership: Ownership): boolean {
		return branch.files.every((f) =>
			f.hunks.every((h) => selectedOwnership.containsHunk(f.id, h.id))
		);
	}

	$: checked = isAllChecked($selectedOwnership);

	function isIndeterminate(selectedOwnership: Ownership): boolean {
		if (branch.files.length <= 1) return false;

		let file = branch.files[0];
		let prev = selectedOwnership.containsHunk(file.id, ...file.hunkIds);
		for (let i = 1; i < branch.files.length; i++) {
			file = branch.files[i];
			const contained = selectedOwnership.containsHunk(file.id, ...file.hunkIds);
			if (contained != prev) {
				return true;
			}
		}
		return false;
	}

	$: indeterminate = isIndeterminate($selectedOwnership);

	function selectAll(selectedOwnership: Writable<Ownership>, files: File[]) {
		files.forEach((f) =>
			selectedOwnership.update((ownership) => ownership.addHunk(f.id, ...f.hunks.map((h) => h.id)))
		);
	}
</script>

{#if branch.active && branch.conflicted}
	<div class="mb-2 bg-red-500 p-2 font-bold text-white">
		{#if branch.files.some((f) => f.conflicted)}
			This virtual branch conflicts with upstream changes. Please resolve all conflicts and commit
			before you can continue.
		{:else}
			Please commit your resolved conflicts to continue.
		{/if}
	</div>
{/if}

<div class="branch-files" class:readonly>
	<div class="header" bind:this={headerElement}>
		<div class="header__left">
			{#if showCheckboxes && selectedListMode == 'list' && branch.files.length > 1}
				<Checkbox
					small
					{checked}
					{indeterminate}
					on:change={(e) => {
						if (e.detail) {
							selectAll(selectedOwnership, branch.files);
						} else {
							selectedOwnership.update((ownership) => ownership.clear());
						}
					}}
				/>
			{/if}
			<div class="header__title text-base-13 text-semibold">
				<span>Changes</span>
				<Badge count={branch.files.length} />
			</div>
		</div>
		<SegmentedControl bind:selected={selectedListMode} selectedIndex={0}>
			<Segment id="list" icon="list-view" />
			<Segment id="tree" icon="tree-view" />
		</SegmentedControl>
	</div>
	{#if branch.files.length > 0}
		<div class="scroll-container">
			<!-- TODO: This is an experiment in file sorting. Accept or reject! -->
			{#if selectedListMode == 'list'}
				<BranchFilesList {branch} {selectedOwnership} {selectedFiles} {showCheckboxes} {readonly} />
			{:else}
				<FileTree
					node={filesToFileTree(branch.files)}
					{showCheckboxes}
					branchId={branch.id}
					isRoot={true}
					{selectedOwnership}
					{selectedFiles}
					{readonly}
				/>
			{/if}
		</div>
	{/if}
</div>

<style lang="postcss">
	.branch-files {
		background: var(--clr-theme-container-light);
		border-radius: var(--radius-m) var(--radius-m) 0 0;
		&.readonly {
			border-radius: var(--radius-m);
		}
	}
	.scroll-container {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding-top: 0;
		padding-left: var(--space-12);
		padding-right: var(--space-12);
		padding-bottom: var(--space-16);
	}
	.header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-top: var(--space-12);
		padding-bottom: var(--space-12);
		padding-left: var(--space-20);
		padding-right: var(--space-12);
		border-color: var(--clr-theme-container-outline-light);
	}
	.header__title {
		display: flex;
		align-items: center;
		gap: var(--space-4);
		color: var(--clr-theme-scale-ntrl-0);
	}
	.header__left {
		display: flex;
		gap: var(--space-10);
	}
</style>
