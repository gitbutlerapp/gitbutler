<script lang="ts">
	import HunkViewer from '$lib/hunk/HunkViewer.svelte';
	import Icon from '$lib/shared/Icon.svelte';
	import LargeDiffMessage from '$lib/shared/LargeDiffMessage.svelte';
	import { computeAddedRemovedByHunk } from '$lib/utils/metrics';
	import { getLocalCommits, getLocalAndRemoteCommits } from '$lib/vbranches/contexts';
	import { getLockText } from '$lib/vbranches/tooltip';
	import { tooltip } from '@gitbutler/ui/utils/tooltip';
	import type { HunkSection, ContentSection } from '$lib/utils/fileSections';

	interface Props {
		filePath: string;
		isBinary: boolean;
		isLarge: boolean;
		sections: (HunkSection | ContentSection)[];
		isUnapplied: boolean;
		selectable: boolean;
		isFileLocked: boolean;
		readonly: boolean;
	}

	let {
		filePath,
		isBinary,
		isLarge,
		sections,
		isUnapplied,
		selectable = false,
		isFileLocked = false,
		readonly = false
	}: Props = $props();

	const localCommits = isFileLocked ? getLocalCommits() : undefined;
	const remoteCommits = isFileLocked ? getLocalAndRemoteCommits() : undefined;

	const commits = isFileLocked ? ($localCommits || []).concat($remoteCommits || []) : undefined;
	let alwaysShow = $state(false);
</script>

<div class="hunks">
	{#if isBinary}
		Binary content not shown
	{:else if isLarge}
		Diff too large to be shown
	{:else if sections.length > 50 && !alwaysShow}
		<LargeDiffMessage
			showFrame
			handleShow={() => {
				alwaysShow = true;
			}}
		/>
	{:else}
		{#each sections as section}
			{@const { added, removed } = computeAddedRemovedByHunk(section)}
			{#if 'hunk' in section}
				<div class="hunk-wrapper">
					<div class="indicators text-base-11">
						<span class="added">+{added}</span>
						<span class="removed">-{removed}</span>
						{#if section.hunk.lockedTo && section.hunk.lockedTo.length > 0 && commits}
							<div
								use:tooltip={{
									text: getLockText(section.hunk.lockedTo, commits),
									delay: 500
								}}
							>
								<Icon name="locked-small" color="warning" />
							</div>
						{/if}
						{#if section.hunk.poisoned}
							Can not manage this hunk because it depends on changes from multiple branches
						{/if}
					</div>
					<HunkViewer
						{filePath}
						{section}
						{selectable}
						{isUnapplied}
						{isFileLocked}
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
		padding: 14px;
		gap: 16px;
	}
	.hunk-wrapper {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.indicators {
		display: flex;
		align-items: center;
		gap: 2px;
	}
	.added {
		color: #45b156;
	}
	.removed {
		color: #ff3e00;
	}
</style>
