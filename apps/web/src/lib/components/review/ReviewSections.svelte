<script lang="ts">
	import SectionComponent from './Section.svelte';
	import { ReviewSectionsService } from '$lib/review/reviewSections.svelte';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { getPatchIdableSections } from '@gitbutler/shared/patches/patchCommitsPreview.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Select from '@gitbutler/ui/select/Select.svelte';
	import SelectItem from '@gitbutler/ui/select/SelectItem.svelte';
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
		commitPageHeaderHeight: number;
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
		commitPageHeaderHeight,
		clearLineSelection,
		toggleDiffLine,
		onCopySelection,
		onQuoteSelection
	}: Props = $props();

	const userService = getContext(UserService);
	const reviewSectionsService = getContext(ReviewSectionsService);
	const user = $derived(userService.user);

	const isLoggedIn = $derived(!!$user);

	let isInterdiffBarVisible = $state(false);

	const allOptions = $derived(reviewSectionsService.allOptions(changeId));

	const beforeOptions = $derived(
		allOptions.current
			.slice(0, -1)
			.map((option) => ({ value: option[0].toString(), label: option[1] }))
	);
	const afterOptions = $derived(
		allOptions.current.slice(1).map((option) => ({ value: option[0].toString(), label: option[1] }))
	);

	const selected = $derived(reviewSectionsService.currentSelection(changeId));
	let initialSelection: { selectedBefore: number; selectedAfter: number } | undefined = $state();
	const isInitialSelection = $derived.by(
		() =>
			initialSelection &&
			selected.current &&
			initialSelection.selectedBefore === selected.current.selectedBefore &&
			initialSelection.selectedAfter === selected.current.selectedAfter
	);

	const selectedAfter = $derived(selected.current?.selectedAfter ?? 1);
	const selectedBefore = $derived(selected.current?.selectedBefore ?? -1);

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

	$effect(() => {
		if (selected.current && !initialSelection) {
			initialSelection = {
				selectedBefore: selected.current.selectedBefore,
				selectedAfter: selected.current.selectedAfter
			};
		}
	});
</script>

<div class="review-sections-card" style:--commit-header-height="{commitPageHeaderHeight}px">
	<div class="review-sections-statistics-wrap">
		<div class="review-sections-statistics">
			<div class="review-sections-statistics__metadata">
				<p class="text-12 text-bold statistic-files">
					{patchCommit.statistics.fileCount} files changed
				</p>
				<p class="text-12 statistic-added">
					{patchCommit.statistics.lines - patchCommit.statistics.deletions} additions
				</p>
				<p class="text-12 statistic-deleted">{patchCommit.statistics.deletions} deletions</p>
			</div>
			<div class="review-sections-statistics__actions">
				<div class="review-sections-statistics__actions__interdiff">
					{#if !isInitialSelection}
						<div class="review-sections-statistics__actions__interdiff-changed"></div>
					{/if}
					<Button
						tooltip="Show interdiff"
						kind="ghost"
						icon={isInterdiffBarVisible ? 'interdiff-fill' : 'interdiff'}
						onclick={() => (isInterdiffBarVisible = !isInterdiffBarVisible)}
					/>
				</div>
			</div>
		</div>
	</div>

	{#if isInterdiffBarVisible}
		<div class="interdiff-bar">
			<p class="text-12 text-bold">Compare versions:</p>

			<div class="interdiff-bar__selects">
				<Select
					searchable
					options={beforeOptions}
					value={selectedBefore.toString()}
					onselect={(value) => {
						reviewSectionsService.setSelection(changeId, {
							selectedBefore: parseInt(value)
						});
					}}
					autoWidth
					popupAlign="right"
				>
					{#snippet customSelectButton()}
						<Button kind="outline" icon="select-chevron" size="tag">
							{beforeOptions.find((option) => option.value === selectedBefore.toString())?.label}
						</Button>
					{/snippet}
					{#snippet itemSnippet({ item, highlighted })}
						{@const isSelected = item.value === selectedBefore.toString()}
						<SelectItem
							selected={isSelected}
							{highlighted}
							disabled={!isSelected && item.value >= selectedAfter.toString()}
						>
							{item.label}
						</SelectItem>
					{/snippet}
				</Select>

				<div class="interdiff-bar__arrow">→</div>

				<Select
					searchable
					options={afterOptions}
					value={selectedAfter.toString()}
					onselect={(value) => {
						reviewSectionsService.setSelection(changeId, {
							selectedAfter: parseInt(value)
						});
					}}
					autoWidth
					popupAlign="right"
				>
					{#snippet customSelectButton()}
						<Button kind="outline" icon="select-chevron" size="tag">
							{afterOptions.find((option) => option.value === selectedAfter.toString())?.label}
						</Button>
					{/snippet}
					{#snippet itemSnippet({ item, highlighted })}
						{@const isSelected = item.value === selectedAfter.toString()}
						<SelectItem
							selected={isSelected}
							{highlighted}
							disabled={!isSelected && item.value <= selectedBefore.toString()}
						>
							{item.label}
						</SelectItem>
					{/snippet}
				</Select>

				{#if !isInitialSelection}
					<Button
						kind="ghost"
						icon="undo-small"
						size="tag"
						tooltip="Reset to initial selection"
						onclick={() => {
							if (initialSelection) {
								reviewSectionsService.setSelection(changeId, initialSelection);
							}
						}}
					>
						Reset
					</Button>
				{/if}
			</div>
		</div>
	{/if}

	<div class="review-sections-diffs">
		<Loading loadable={patchSections?.current}>
			{#snippet children(patchSections)}
				{#each patchSections || [] as section}
					<SectionComponent
						{isLoggedIn}
						{section}
						{toggleDiffLine}
						{selectedSha}
						{selectedLines}
						{commitPageHeaderHeight}
						{onCopySelection}
						{onQuoteSelection}
						{clearLineSelection}
					/>
				{/each}
			{/snippet}
		</Loading>
	</div>
</div>

<style>
	.review-sections-card {
		display: flex;
		flex-direction: column;
		contain: paint;
	}

	.review-sections-statistics-wrap {
		display: flex;
		width: 100%;
	}

	.review-sections-statistics {
		width: 100%;
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 10px 10px 10px 14px;
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		border-top-left-radius: var(--radius-ml);
		border-top-right-radius: var(--radius-ml);
	}

	.review-sections-statistics__metadata {
		display: flex;
		gap: 8px;
	}

	.review-sections-statistics__actions {
		display: flex;
		gap: 2px;
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

	/* INTERDIFF */

	.interdiff-bar {
		display: flex;
		gap: 12px;
		align-items: center;

		background-color: var(--clr-bg-1-muted);
		width: 100%;

		border: 1px solid var(--clr-border-2);
		border-top: none;

		padding: 14px;

		@container (max-width: 500px) {
			flex-direction: column;
			align-items: flex-start;
			gap: 8px;
		}
	}

	.interdiff-bar__selects {
		display: flex;
		gap: 6px;
	}

	.interdiff-bar__arrow {
		color: var(--clr-text-2);
	}

	.review-sections-statistics__actions {
		display: flex;
		gap: 2px;
	}

	.review-sections-statistics__actions__interdiff {
		position: relative;
		display: flex;
	}

	.review-sections-statistics__actions__interdiff-changed {
		position: absolute;
		top: 2px;
		right: 2px;
		width: 7px;
		height: 7px;
		background-color: var(--clr-theme-pop-element);
		border-radius: 50%;
	}
</style>
