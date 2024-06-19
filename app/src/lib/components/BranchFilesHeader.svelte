<script lang="ts">
	import Badge from '$lib/components/Badge.svelte';
	import Checkbox from '$lib/components/Checkbox.svelte';
	import { maybeGetContextStore } from '$lib/utils/context';
	import { Ownership } from '$lib/vbranches/ownership';
	import type { AnyFile } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let title: string;
	export let files: AnyFile[];
	export let showCheckboxes = false;

	const selectedOwnership: Writable<Ownership> | undefined = maybeGetContextStore(Ownership);

	function selectAll(files: AnyFile[]) {
		if (!selectedOwnership) return;
		files.forEach((f) => selectedOwnership.update((ownership) => ownership.add(f.id, ...f.hunks)));
	}

	function isAllChecked(selectedOwnership: Ownership | undefined): boolean {
		if (!selectedOwnership) return false;
		return files.every((f) => f.hunks.every((h) => selectedOwnership.contains(f.id, h.id)));
	}

	function isIndeterminate(selectedOwnership: Ownership | undefined): boolean {
		if (!selectedOwnership) return false;
		if (files.length <= 1) return false;

		let file = files[0];
		let prev = selectedOwnership.contains(file.id, ...file.hunkIds);
		for (let i = 1; i < files.length; i++) {
			file = files[i];
			const contained = selectedOwnership.contains(file.id, ...file.hunkIds);
			if (contained !== prev) {
				return true;
			}
		}
		return false;
	}

	$: indeterminate = selectedOwnership ? isIndeterminate($selectedOwnership) : false;
	$: checked = isAllChecked($selectedOwnership);
</script>

<div class="header">
	<div class="header__left">
		{#if showCheckboxes && files.length > 1}
			<Checkbox
				small
				{checked}
				{indeterminate}
				on:change={(e) => {
					if (e.detail) {
						selectAll(files);
					} else {
						selectedOwnership?.update((ownership) => ownership.clear());
					}
				}}
			/>
		{/if}
		<div class="header__title text-base-13 text-semibold">
			<span>{title}</span>
			<Badge count={files.length} />
		</div>
	</div>
</div>

<style lang="postcss">
	.header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 14px;
		border-bottom: none;
		border-radius: var(--radius-m) var(--radius-m) 0 0;
		background-color: var(--clr-bg-1);
	}
	.header__title {
		display: flex;
		align-items: center;
		gap: 4px;
		color: var(--clr-scale-ntrl-0);
	}
	.header__left {
		display: flex;
		gap: 10px;
	}
</style>
