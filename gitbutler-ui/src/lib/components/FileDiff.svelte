<script lang="ts">
	import HunkViewer from './HunkViewer.svelte';
	import Icon from './Icon.svelte';
	import { computeAddedRemovedByHunk } from '$lib/utils/metrics';
	import type { HunkSection, ContentSection } from '$lib/utils/fileSections';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { Writable } from 'svelte/store';

	export let branchId: string | undefined;
	export let filePath: string;
	export let isBinary: boolean;
	export let isLarge: boolean;
	export let sections: (HunkSection | ContentSection)[];
	export let projectPath: string | undefined;
	export let branchController: BranchController;
	export let isUnapplied: boolean;
	export let selectable = false;
	export let selectedOwnership: Writable<Ownership> | undefined = undefined;
	export let isFileLocked = false;
	export let readonly: boolean = false;

	function getGutterMinWidth(max: number) {
		if (max >= 10000) return 2.5;
		if (max >= 1000) return 2;
		if (max >= 100) return 1.5;
		if (max >= 10) return 1.25;
		return 1;
	}

	$: maxLineNumber = sections[sections.length - 1]?.maxLineNumber;
	$: minWidth = getGutterMinWidth(maxLineNumber);
</script>

<div class="hunks">
	{#if isBinary}
		Binary content not shown
	{:else if isLarge}
		Diff too large to be shown
	{:else}
		{#each sections as section}
			{@const { added, removed } = computeAddedRemovedByHunk(section)}
			{#if 'hunk' in section}
				<div class="hunk-wrapper">
					<div class="indicators text-base-11">
						<span class="added">+{added}</span>
						<span class="removed">-{removed}</span>
						{#if section.hunk.locked}
							<div title={section.hunk.lockedTo}>
								<Icon name="locked-small" color="warn" />
							</div>
						{/if}
					</div>
					<HunkViewer
						{filePath}
						{section}
						{branchId}
						{selectable}
						{isUnapplied}
						{projectPath}
						{selectedOwnership}
						{branchController}
						{isFileLocked}
						{minWidth}
						{readonly}
						linesModified={added + removed}
					/>
				</div>
			{/if}
		{/each}
	{/if}
</div>

<style lang="postcss">
	.hunks {
		display: flex;
		flex-direction: column;
		position: relative;
		max-height: 100%;
		flex-shrink: 0;
		padding: var(--space-16);
		gap: var(--space-16);
	}
	.hunk-wrapper {
		display: flex;
		flex-direction: column;
		gap: var(--space-10);
	}
	.indicators {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}
	.added {
		color: #45b156;
	}
	.removed {
		color: #ff3e00;
	}
</style>
