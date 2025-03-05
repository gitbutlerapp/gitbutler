<script lang="ts">
	import SectionComponent from './Section.svelte';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import type { Patch, Section } from '@gitbutler/shared/patches/types';
	import type { LineClickParams } from '@gitbutler/ui/HunkDiff.svelte';
	import type { ContentSection, LineSelector } from '@gitbutler/ui/utils/diffParsing';

	interface Props {
		patch: Patch;
		patchSections: Section[] | undefined;
		selectedSha: string | undefined;
		selectedLines: LineSelector[];
		headerShift: number | undefined;
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
		headerShift,
		clearLineSelection,
		toggleDiffLine,
		onCopySelection,
		onQuoteSelection
	}: Props = $props();

	const userService = getContext(UserService);
	const user = $derived(userService.user);

	const isLoggedIn = $derived(!!$user);

	let offsetHeight = $state(0);

	$effect(() => {
		if (headerShift) {
			offsetHeight = headerShift;
		}
	});
</script>

<div class="review-sections-card">
	<div class="review-sections-statistics-wrap" style:--header-shift="{offsetHeight}px">
		<div class="review-sections-statistics">
			<p class="text-12 text-bold statistic-files">{patch.statistics.fileCount} files changed</p>
			<p class="text-12 statistic-added">
				{patch.statistics.lines - patch.statistics.deletions} additions
			</p>
			<p class="text-12 statistic-deleted">{patch.statistics.deletions} deletions</p>
		</div>
	</div>

	<div class="review-sections-diffs">
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
</div>

<style>
	.review-sections-card {
		position: relative;
		display: flex;
		flex-direction: column;
	}

	.review-sections-statistics-wrap {
		position: relative;
		display: flex;
		width: 100%;
		z-index: var(--z-ground);
		position: sticky;
		top: var(--header-shift, 0);

		&::after {
			content: '';
			position: absolute;
			top: 0;
			left: 0;
			width: 100%;
			height: 20px;
			z-index: -1;
			background-color: var(--clr-bg-2);
		}
	}

	.review-sections-statistics {
		height: 48px;
		width: 100%;
		display: flex;
		gap: 8px;
		padding: 17px 16px;
		align-items: center;
		align-self: stretch;
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		border-top-left-radius: var(--radius-ml);
		border-top-right-radius: var(--radius-ml);
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

	.review-sections-diffs {
		position: relative;
		display: flex;
		flex-direction: column;
		width: 100%;
	}
</style>
