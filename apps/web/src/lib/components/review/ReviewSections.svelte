<script lang="ts">
	import SectionComponent from './Section.svelte';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import type { Patch, Section } from '@gitbutler/shared/branches/types';
	import type { LineClickParams } from '@gitbutler/ui/HunkDiff.svelte';
	import type { ContentSection, LineSelector } from '@gitbutler/ui/utils/diffParsing';

	interface Props {
		patch: Patch;
		patchSections: Section[] | undefined;
		selectedSha: string | undefined;
		selectedLines: LineSelector[];
		clearLineSelection: (fileName: string) => void;
		toggleDiffLine: (fileName: string, diffSha: string, params: LineClickParams) => void;
		onCopySelection: (contentSections: ContentSection[]) => void;
		onQuoteSelection: () => void;
	}

	const {
		patch,
		patchSections,
		selectedSha,
		selectedLines,
		clearLineSelection,
		toggleDiffLine,
		onCopySelection,
		onQuoteSelection
	}: Props = $props();

	const userService = getContext(UserService);
	const user = $derived(userService.user);

	const isLoggedIn = $derived(!!$user);
</script>

<div class="review-sections-card">
	<div class="review-sections-statistics">
		<p class="text-12 text-bold statistic-files">{patch.statistics.fileCount} files changed</p>
		<p class="text-12 statistic-added">
			{patch.statistics.lines - patch.statistics.deletions} additions
		</p>
		<p class="text-12 statistic-deleted">{patch.statistics.deletions} deletions</p>
	</div>
	{#if patchSections !== undefined}
		{#each patchSections as section}
			<SectionComponent
				{isLoggedIn}
				{section}
				{toggleDiffLine}
				{selectedSha}
				{selectedLines}
				{onCopySelection}
				{onQuoteSelection}
				{clearLineSelection}
			/>
		{/each}
	{/if}
</div>

<style>
	.review-sections-card {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		align-self: stretch;

		border-radius: var(--radius-ml);
		border: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
	}

	.review-sections-statistics {
		height: 48px;
		display: flex;
		gap: 8px;
		padding: 17px 16px;
		align-items: center;
		align-self: stretch;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.statistic-files {
		color: var(--clr-text-1);
	}

	.statistic-added {
		color: var(--clr-scale-succ-30);
	}

	.statistic-deleted {
		color: var(--clr-scale-err-30);
	}
</style>
