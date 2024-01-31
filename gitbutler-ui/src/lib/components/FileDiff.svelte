<script lang="ts">
	import HunkViewer from './HunkViewer.svelte';
	import Icon from './Icon.svelte';
	import { computeAddedRemovedByFiles, computeAddedRemovedByHunk } from '$lib/utils/metrics';
	import type { HunkSection, ContentSection } from '$lib/utils/fileSections';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { File } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let branchId: string;
	export let file: File;
	export let sections: (HunkSection | ContentSection)[] = [];
	export let projectPath: string | undefined;
	export let branchController: BranchController;
	export let isUnapplied: boolean;
	export let selectable = false;
	export let selectedOwnership: Writable<Ownership>;
	export let isFileLocked: boolean;
</script>

<div class="hunks">
	{#if file.binary}
		Binary content not shown
	{:else if file.large}
		Diff too large to be shown
	{:else}
		{#each sections as section}
			{@const { added, removed } = computeAddedRemovedByHunk(section)}
			{#if 'hunk' in section}
				<div class="hunk-wrapper">
					<div class="indicators text-base-11">
						<span class="added">+{added}</span>
						<span class="removed">+{removed}</span>
						{#if section.hunk.locked}
							<div title={section.hunk.lockedTo}>
								<Icon name="locked-small" color="warn" />
							</div>
						{/if}
					</div>
					<HunkViewer
						{file}
						{section}
						{branchId}
						{selectable}
						{isUnapplied}
						{projectPath}
						{selectedOwnership}
						{branchController}
						{isFileLocked}
						{minWidth}
					/>
				</div>
			{/if}
		{/each}
	{/if}
</div>
