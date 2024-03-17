<script lang="ts">
	import Badge from '$lib/components/Badge.svelte';
	import Checkbox from '$lib/components/Checkbox.svelte';
	import Segment from '$lib/components/SegmentControl/Segment.svelte';
	import SegmentedControl from '$lib/components/SegmentControl/SegmentedControl.svelte';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { AnyFile } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let files: AnyFile[];
	export let selectedOwnership: Writable<Ownership>;
	export let showCheckboxes = false;

	export let selectedListMode: string;

	function selectAll(selectedOwnership: Writable<Ownership>, files: AnyFile[]) {
		files.forEach((f) =>
			selectedOwnership.update((ownership) => ownership.addHunk(f.id, ...f.hunks.map((h) => h.id)))
		);
	}

	function isAllChecked(selectedOwnership: Ownership): boolean {
		return files.every((f) => f.hunks.every((h) => selectedOwnership.containsHunk(f.id, h.id)));
	}

	function isIndeterminate(selectedOwnership: Ownership): boolean {
		if (files.length <= 1) return false;

		let file = files[0];
		let prev = selectedOwnership.containsHunk(file.id, ...file.hunkIds);
		for (let i = 1; i < files.length; i++) {
			file = files[i];
			const contained = selectedOwnership.containsHunk(file.id, ...file.hunkIds);
			if (contained != prev) {
				return true;
			}
		}
		return false;
	}

	$: indeterminate = isIndeterminate($selectedOwnership);
	$: checked = isAllChecked($selectedOwnership);
</script>

<div class="header">
	<div class="header__left">
		{#if showCheckboxes && selectedListMode == 'list' && files.length > 1}
			<Checkbox
				small
				{checked}
				{indeterminate}
				on:change={(e) => {
					if (e.detail) {
						selectAll(selectedOwnership, files);
					} else {
						selectedOwnership.update((ownership) => ownership.clear());
					}
				}}
			/>
		{/if}
		<div class="header__title text-base-13 text-semibold">
			<span>Changes</span>
			<Badge count={files.length} />
		</div>
	</div>
	<SegmentedControl bind:selected={selectedListMode} selectedIndex={0}>
		<Segment id="list" icon="list-view" />
		<Segment id="tree" icon="tree-view" />
	</SegmentedControl>
</div>

<style lang="postcss">
	.header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}
	.header__title {
		display: flex;
		align-items: center;
		gap: var(--size-4);
		color: var(--clr-theme-scale-ntrl-0);
	}
	.header__left {
		display: flex;
		gap: var(--size-10);
	}
</style>
