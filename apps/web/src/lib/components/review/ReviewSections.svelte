<script lang="ts">
	import SectionComponent from './Section.svelte';
	import { ReviewSectionsService } from '$lib/review/reviewSections.svelte';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import { getPatchIdableSections } from '@gitbutler/shared/patches/patchCommitsPreview.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import DropDownButton from '@gitbutler/ui/DropDownButton.svelte';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import type { PatchCommit } from '@gitbutler/shared/patches/types';
	import type { LineClickParams } from '@gitbutler/ui/HunkDiff.svelte';
	import type { ContentSection, LineSelector } from '@gitbutler/ui/utils/diffParsing';

	interface Props {
		branchUuid: string;
		changeId: string;
		patchCommit: PatchCommit;
		selectedSha: string | undefined;
		selectedLines: LineSelector[];
		headerShift: number | undefined;
		clearLineSelection: (fileName: string) => void;
		toggleDiffLine: (fileName: string, diffSha: string, params: LineClickParams) => void;
		onCopySelection: (contentSections: ContentSection[]) => void;
		onQuoteSelection: () => void;
	}

	const {
		branchUuid,
		changeId,
		patchCommit,
		selectedSha,
		selectedLines,
		headerShift,
		clearLineSelection,
		toggleDiffLine,
		onCopySelection,
		onQuoteSelection
	}: Props = $props();

	const userService = getContext(UserService);
	const reviewSectionsService = getContext(ReviewSectionsService);
	const user = $derived(userService.user);

	const isLoggedIn = $derived(!!$user);

	let offsetHeight = $state(0);

	$effect(() => {
		if (headerShift) {
			offsetHeight = headerShift;
		}
	});

	const allOptions = $derived(reviewSectionsService.allOptions(branchUuid, changeId));

	const beforeOptions = $derived(allOptions.current.slice(0, -1));
	const afterOptions = $derived(allOptions.current.slice(1));

	const selected = $derived(reviewSectionsService.currentSelection(branchUuid, changeId));

	const selectedAfter = $derived(selected.current?.selectedAfter ?? 1);
	const selectedBefore = $derived(selected.current?.selectedBefore ?? -1);

	let beforeButton = $state<DropDownButton>();
	let afterButton = $state<DropDownButton>();

	const patchSections = $derived(
		isDefined(selectedAfter)
			? getPatchIdableSections(
					branchUuid,
					changeId,
					selectedBefore === -1 ? undefined : selectedBefore,
					selectedAfter
				)
			: undefined
	);
</script>

<div class="review-sections-card">
	<div class="review-sections-statistics-wrap" style:--header-shift="{offsetHeight}px">
		<div class="review-sections-statistics">
			<p class="text-12 text-bold statistic-files">
				{patchCommit.statistics.fileCount} files changed
			</p>
			<p class="text-12 statistic-added">
				{patchCommit.statistics.lines - patchCommit.statistics.deletions} additions
			</p>
			<p class="text-12 statistic-deleted">{patchCommit.statistics.deletions} deletions</p>
		</div>
	</div>
	<div class="interdiff-bar">
		<p class="text-12 text-bold">Compare the versions:</p>
		<DropDownButton bind:this={beforeButton} kind="outline">
			{beforeOptions.find((beforeOption) => beforeOption[0] === selectedBefore)?.[1]}

			{#snippet contextMenuSlot()}
				<ContextMenuSection>
					{#each beforeOptions as option}
						<ContextMenuItem
							label={option[1]}
							disabled={option[0] >= (selectedAfter || 0)}
							onclick={() => {
								reviewSectionsService.setSelection(branchUuid, changeId, {
									selectedBefore: option[0]
								});
								beforeButton?.close();
							}}
						/>
					{/each}
				</ContextMenuSection>
			{/snippet}
		</DropDownButton>
		<DropDownButton bind:this={afterButton} kind="outline">
			{afterOptions.find((afterOption) => afterOption[0] === selectedAfter)?.[1]}

			{#snippet contextMenuSlot()}
				<ContextMenuSection>
					{#each afterOptions as option}
						<ContextMenuItem
							label={option[1]}
							disabled={option[0] <= (selectedBefore || 0)}
							onclick={() => {
								reviewSectionsService.setSelection(branchUuid, changeId, {
									selectedAfter: option[0]
								});
								afterButton?.close();
							}}
						/>
					{/each}
				</ContextMenuSection>
			{/snippet}
		</DropDownButton>
	</div>

	<div class="review-sections-diffs">
		{#if patchSections !== undefined}
			{#each patchSections.current || [] as section}
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

	.interdiff-bar {
		background-color: var(--clr-bg-1);
		width: 100%;

		border: 1px solid var(--clr-border-2);
		border-top: none;

		padding: 16px;

		display: flex;
		gap: 12px;
		align-items: center;
	}
</style>
