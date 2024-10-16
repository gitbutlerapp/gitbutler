<script lang="ts">
	import { invoke } from '$lib/backend/ipc';
	import { Project } from '$lib/backend/projects';
	import HunkViewer from '$lib/hunk/HunkViewer.svelte';
	import InfoMessage from '$lib/shared/InfoMessage.svelte';
	import LargeDiffMessage from '$lib/shared/LargeDiffMessage.svelte';
	import { computeAddedRemovedByHunk } from '$lib/utils/metrics';
	import { getLocalCommits, getLocalAndRemoteCommits } from '$lib/vbranches/contexts';
	import { getLockText } from '$lib/vbranches/tooltip';
	import { getContext } from '@gitbutler/shared/context';
	import type { HunkSection, ContentSection } from '$lib/utils/fileSections';

	interface FileInfo {
		content: string;
		name?: string;
		mimeType?: string;
		size?: number;
	}

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

	let alwaysShow = $state(false);
	const project = getContext(Project);
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

	const KB = 1024;
	const MB = 1024 ** 2;
	const GB = 1024 ** 3;

	function formatFileSize(bytes: number): string {
		if (bytes < KB) return bytes + ' B';
		else if (bytes < MB) return (bytes / KB).toFixed(1) + ' KB';
		else if (bytes < GB) return (bytes / MB).toFixed(1) + ' MB';
		else return (bytes / GB).toFixed(1) + ' GB';
	}

	let fileInfo: FileInfo = $state({
		content: '',
		name: undefined,
		mimeType: undefined,
		size: 0
	});

	async function fetchBlobInfo() {
		try {
			const fetchedFileInfo: FileInfo = await invoke('get_blob_info', {
				relativePath: filePath,
				projectId: project.id,
				commitId
			});
			fileInfo = fetchedFileInfo;

			// If file.size > 5mb; don't render it
			if (fileInfo.size && fileInfo.size > 5 * 1024 * 1024) {
				isLarge = true;
			}
		} catch (error) {
			console.error(error);
		}
	}

	$effect(() => {
		fetchBlobInfo();
	});
</script>

<div class="hunks">
	{#if isLarge}
		Change too large to be shown
	{:else if isBinary}
		{#if fileInfo.mimeType && fileInfo.content}
			<img
				class="hunk-image"
				src="data:{fileInfo.mimeType};base64,{fileInfo.content}"
				oncontextmenu={(e) => {
					e.preventDefault();
				}}
				alt={fileInfo.name}
			/>
		{/if}
		{#if fileInfo.size === undefined}
			<p>File has been deleted</p>
		{:else}
			<p class="hunk-label__size">{formatFileSize(fileInfo.size || 0)}</p>
		{/if}
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
				{@const isHunkLocked = section.hunk.lockedTo && section.hunk.lockedTo.length > 0 && commits}
				<div class="hunk-wrapper">
					{#if isHunkLocked || section.hunk.poisoned}
						<div class="indicators text-11 text-semibold">
							{#if isHunkLocked}
								<InfoMessage filled outlined={false} style="warning" icon="locked">
									<svelte:fragment slot="content">
										{getLockText(section.hunk.lockedTo, commits)}
									</svelte:fragment>
								</InfoMessage>
							{/if}
							{#if section.hunk.poisoned}
								<InfoMessage filled outlined={false}>
									<svelte:fragment slot="content">
										Can not manage this hunk because it depends on changes from multiple branches
									</svelte:fragment>
								</InfoMessage>
							{/if}
						</div>
					{/if}
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

	.hunk-image {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.hunk-label__size {
		align-self: flex-start;
		font-size: 12px;
		color: var(--clr-text-3);
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
	img {
		max-width: 100%;
		height: auto;
	}
	p {
		display: flex;
		flex-direction: column;
		align-items: center;
	}
</style>
