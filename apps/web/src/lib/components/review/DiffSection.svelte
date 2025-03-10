<script lang="ts">
	import { isLockfile } from '$lib/diff/lockfiles';
	import { splitDiffIntoHunks } from '$lib/diffParsing';
	import { getFilePathInfo } from '@gitbutler/shared/utils/file';
	import Button from '@gitbutler/ui/Button.svelte';
	import HunkDiff, { type LineClickParams } from '@gitbutler/ui/HunkDiff.svelte';
	import FileIcon from '@gitbutler/ui/file/FileIcon.svelte';
	import type { DiffSection } from '@gitbutler/shared/patches/types';
	import type { ContentSection, LineSelector } from '@gitbutler/ui/utils/diffParsing';

	interface Props {
		isLoggedIn: boolean;
		section: DiffSection;
		selectedSha: string | undefined;
		selectedLines: LineSelector[];
		commitPageHeaderHeight: number;
		clearLineSelection: (fileName: string) => void;
		toggleDiffLine: (fileName: string, diffSha: string, params: LineClickParams) => void;
		onCopySelection: (contentSections: ContentSection[]) => void;
		onQuoteSelection: () => void;
	}
	const {
		isLoggedIn,
		section,
		toggleDiffLine,
		selectedSha,
		selectedLines: lines,
		commitPageHeaderHeight,
		onCopySelection,
		onQuoteSelection,
		clearLineSelection
	}: Props = $props();

	const lockFile = $derived.by(() => {
		if (!section.newPath) return false;
		return isLockfile(section.newPath);
	});

	let displayLockHunks = $state<boolean>(false);

	const hunks = $derived.by(() => {
		if (!section.diffPatch) return [];
		return splitDiffIntoHunks(section.diffPatch);
	});
	const filePath = $derived(section.newPath || 'unknown');
	const filePathInfo = $derived(getFilePathInfo(filePath));

	function handleLineClick(params: LineClickParams) {
		toggleDiffLine(section.newPath || 'unknown', section.diffSha, params);
	}

	const selectedLines = $derived(selectedSha === section.diffSha ? lines : []);

	let diffFolded = $state(false);

	let diffHeaderEl: HTMLDivElement | null = $state(null);
	let isHeaderSticked = $state(false);
	let headerHeight = $state(0);
	let windowHeight = $state(0);

	$effect(() => {
		if (!diffHeaderEl || !commitPageHeaderHeight) return;

		const observer = new IntersectionObserver(
			(entries) => {
				entries.forEach((entry) => {
					isHeaderSticked = entry.isIntersecting;
				});
			},
			{
				root: null,
				rootMargin: `0px 0px -${windowHeight - commitPageHeaderHeight}px`,
				threshold: 0
			}
		);

		observer.observe(diffHeaderEl);

		return () => {
			observer.disconnect();
		};
	});
</script>

<svelte:window bind:innerHeight={windowHeight} />

<div
	class="diff-section__header"
	bind:this={diffHeaderEl}
	bind:clientHeight={headerHeight}
	class:diff-section__header_sticked={isHeaderSticked}
>
	<div class="diff-section__header__file-info">
		<FileIcon fileName={filePath} size={16} />
		<p title={filePath} class="text-12 text-body file-path">
			{#if filePathInfo}
				<span class="directory-path">{filePathInfo.directoryPath}/</span>{filePathInfo.fileName}
			{:else}
				{filePath}
			{/if}
		</p>
	</div>

	<div class="diff-section__header__fold-button" class:folded={!diffFolded}>
		<Button
			kind="ghost"
			size="tag"
			icon={diffFolded ? 'chevron-down' : 'chevron-up'}
			onclick={() => (diffFolded = !diffFolded)}
		/>
	</div>
</div>

{#if !diffFolded}
	<div class="diff-section__content">
		{#if lockFile && !displayLockHunks}
			<div class="lock-files-hidden-by-default">
				<p class="text-12 hidden-lock-file-message">Lock files are hidden by default</p>
				<Button kind="outline" icon="eye-shown" onclick={() => (displayLockHunks = true)}
					>Show diff</Button
				>
			</div>
		{:else}
			{#each hunks as hunkStr}
				<HunkDiff
					filePath={section.newPath || 'unknown'}
					{hunkStr}
					diffLigatures={false}
					{selectedLines}
					onLineClick={handleLineClick}
					{onCopySelection}
					onQuoteSelection={isLoggedIn ? onQuoteSelection : undefined}
					{clearLineSelection}
				/>
			{/each}
		{/if}
	</div>
{/if}

<style lang="postcss">
	.diff-section__header {
		top: 0;
		position: sticky;
		z-index: 1;
		top: var(--commit-header-height, 0);
		width: 100%;
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 8px;
		padding: 14px 12px 14px 14px;
		background-color: var(--clr-bg-1);
		border-left: 1px solid var(--clr-border-2);
		border-right: 1px solid var(--clr-border-2);
		border-top: 1px solid var(--clr-border-2);
		margin-top: -1px;

		&:hover {
			.diff-section__header__fold-button {
				opacity: 1;
			}
		}
	}

	.diff-section__header__file-info {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.diff-section__header__fold-button {
		display: flex;

		transition: opacity var(--transition-fast);

		&.folded {
			opacity: 0;
		}
	}

	.diff-section__content {
		width: 100%;
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 8px;
		padding: 0 14px 14px 14px;
		background-color: var(--clr-bg-1);
		border-left: 1px solid var(--clr-border-2);
		border-right: 1px solid var(--clr-border-2);
		border-bottom: 1px solid var(--clr-border-2);

		&:last-child {
			border-bottom-right-radius: var(--radius-ml);
			border-bottom-left-radius: var(--radius-ml);
		}
	}

	.file-path {
		color: var(--clr-text-1);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		direction: rtl;
		text-align: left;

		& span {
			margin: 0;
			padding: 0;
		}
	}

	.directory-path {
		color: var(--clr-text-2);
	}

	.lock-files-hidden-by-default {
		width: 100%;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 14px;
		padding: 40px 24px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background: var(--clr-bg-1-muted);
	}

	.hidden-lock-file-message {
		color: var(--clr-text-2);
		text-align: center;
	}

	/* MODIFIERS */
	.diff-section__header_sticked {
		/* background-color: red; */
		border-bottom: 1px solid var(--clr-border-2);
	}
</style>
