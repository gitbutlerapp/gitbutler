<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import HunkViewer from '$lib/hunk/HunkViewer.svelte';
	import LargeDiffMessage from '$lib/shared/LargeDiffMessage.svelte';
	import { getContext } from '$lib/utils/context';
	import { getFileExtension } from '$lib/utils/filePath';
	import { getBinFileSize } from '$lib/utils/fileSize';
	import { computeAddedRemovedByHunk } from '$lib/utils/metrics';
	import { getLocalCommits, getLocalAndRemoteCommits } from '$lib/vbranches/contexts';
	import { getLockText } from '$lib/vbranches/tooltip';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import { join } from '@tauri-apps/api/path';
	import { convertFileSrc } from '@tauri-apps/api/tauri';
	import { onMount } from 'svelte';
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
		commitId?: string;
	}

	let {
		filePath,
		isBinary,
		isLarge,
		sections,
		isUnapplied,
		commitId,
		selectable = false,
		isFileLocked = false,
		readonly = false
	}: Props = $props();

	let fileSize = $state('...');
	let imageSrc = $state('');
	const project = getContext(Project);

	onMount(async () => {
		if (isBinary || !isLarge) {
			const fullPath = await join(project.path, filePath);
			fileSize = await getBinFileSize(fullPath);
			if (isImage) {
				try {
					imageSrc = convertFileSrc(fullPath);
				} catch (error) {
					console.error('Failed to convert image path:', error);
					console.error('Attempted image source:', fullPath);
				}
			}
		}
	});

	let alwaysShow = $state(false);
	const localCommits = isFileLocked ? getLocalCommits() : undefined;
	const remoteCommits = isFileLocked ? getLocalAndRemoteCommits() : undefined;

	const commits = isFileLocked ? ($localCommits || []).concat($remoteCommits || []) : undefined;

	function getGutterMinWidth(max: number | undefined) {
		if (!max) {
			return 1;
		}
		if (max >= 10000) return 2.5;
		if (max >= 1000) return 2;
		if (max >= 100) return 1.5;
		if (max >= 10) return 1.25;
		return 1;
	}
	const maxLineNumber = $derived(sections.at(-1)?.maxLineNumber);
	const minWidth = $derived(getGutterMinWidth(maxLineNumber));
	const isImage = $derived(
		['png', 'jpg', 'jpeg', 'gif', 'svg'].includes(getFileExtension(filePath))
	);
</script>

<div class="hunks">
	{#if isBinary}
		<div class="binary-info">
			{#if isImage}
				<img src={imageSrc} alt="Binary file" class="binary-image" />
			{:else}
				<p>Binary content not shown</p>
			{/if}
			<p>file size: {fileSize}</p>
		</div>
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
					<div class="indicators text-11 text-semibold">
						<div class="text-10 semibold added-removed">
							<span class="added">+{added}</span>
							<span class="removed">-{removed}</span>
						</div>

						{#if section.hunk.lockedTo && section.hunk.lockedTo.length > 0 && commits}
							<Tooltip text={getLockText(section.hunk.lockedTo, commits)}>
								<Icon name="locked-small" color="warning" />
							</Tooltip>
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
						{minWidth}
						{readonly}
						{commitId}
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

	.binary-image {
		max-width: 100%;
		height: auto;
	}

	.binary-info {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 10px;
	}

	.added-removed {
		display: flex;
		border-radius: var(--radius-s);
		overflow: hidden;
	}

	.removed,
	.added {
		padding: 2px 4px;
	}

	.added {
		color: var(--clr-scale-succ-30);
		background-color: var(--clr-theme-succ-bg);
	}

	.removed {
		color: var(--clr-scale-err-30);
		background-color: var(--clr-theme-err-bg);
	}
</style>
