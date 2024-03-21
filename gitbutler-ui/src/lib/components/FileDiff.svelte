<script lang="ts">
	import Button from './Button.svelte';
	import HunkViewer from './HunkViewer.svelte';
	import Icon from './Icon.svelte';
	import { computeAddedRemovedByHunk } from '$lib/utils/metrics';
	import { tooltip } from '$lib/utils/tooltip';
	import type { HunkSection, ContentSection } from '$lib/utils/fileSections';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { Commit } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let branchId: string | undefined;
	export let filePath: string;
	export let isBinary: boolean;
	export let isLarge: boolean;
	export let sections: (HunkSection | ContentSection)[];
	export let isUnapplied: boolean;
	export let selectable = false;
	export let selectedOwnership: Writable<Ownership> | undefined = undefined;
	export let isFileLocked = false;
	export let readonly: boolean = false;
	export let branchCommits: Commit[];

	function getGutterMinWidth(max: number) {
		if (max >= 10000) return 2.5;
		if (max >= 1000) return 2;
		if (max >= 100) return 1.5;
		if (max >= 10) return 1.25;
		return 1;
	}

	function getLockedTooltip(commitId: string | undefined): string {
		if (!commitId) return 'Depends on a committed change';
		const shortCommitId = commitId?.slice(0, 7);
		const commit = branchCommits.find((commit) => commit.id === commitId);
		if (!commit || !commit.descriptionTitle) return `Depends on commit ${shortCommitId}`;

		const shortTitle = commit.descriptionTitle.slice(0, 35) + '...';
		return `Depends on commit "${shortTitle}" (${shortCommitId})`;
	}

	$: maxLineNumber = sections[sections.length - 1]?.maxLineNumber;
	$: minWidth = getGutterMinWidth(maxLineNumber);

	let alwaysShow = false;
</script>

<div class="hunks">
	{#if isBinary}
		Binary content not shown
	{:else if isLarge}
		Diff too large to be shown
	{:else if sections.length > 50 && !alwaysShow}
		<div class="flex flex-col p-1">
			Change hidden as large numbers of diffs may slow down the UI
			<Button kind="outlined" color="neutral" on:click={() => (alwaysShow = true)}
				>show anyways</Button
			>
		</div>
	{:else}
		{#each sections as section}
			{@const { added, removed } = computeAddedRemovedByHunk(section)}
			{#if 'hunk' in section}
				<div class="hunk-wrapper">
					<div class="indicators text-base-11">
						<span class="added">+{added}</span>
						<span class="removed">-{removed}</span>
						{#if section.hunk.locked}
							<div
								use:tooltip={{
									text: getLockedTooltip(section.hunk.lockedTo),
									delay: 500
								}}
							>
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
						{selectedOwnership}
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
		padding: var(--size-16);
		gap: var(--size-16);
	}
	.hunk-wrapper {
		display: flex;
		flex-direction: column;
		gap: var(--size-10);
	}
	.indicators {
		display: flex;
		align-items: center;
		gap: var(--size-2);
	}
	.added {
		color: #45b156;
	}
	.removed {
		color: #ff3e00;
	}
</style>
