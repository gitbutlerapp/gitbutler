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

	let isInterdiffBarVisible = $state(false);

	$effect(() => {
		if (headerShift) {
			offsetHeight = headerShift;
		}

		if (selected.current && !initialSelection) {
			initialSelection = {
				selectedBefore: selected.current.selectedBefore,
				selectedAfter: selected.current.selectedAfter
			};
		}
	});

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
</script>

<div class="review-sections-card">
	<div class="review-sections-statistics-wrap" style:--header-shift="{offsetHeight}px">
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
					{#if initialSelection && selected.current}
						{#if initialSelection.selectedBefore !== selected.current.selectedBefore || initialSelection.selectedAfter !== selected.current.selectedAfter}
							<div class="review-sections-statistics__actions__interdiff-changed"></div>
						{/if}
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
					<SelectItem selected={item.value === selectedBefore.toString()} {highlighted}>
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
					<SelectItem selected={item.value === selectedAfter.toString()} {highlighted}>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>
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

	.interdiff-bar__arrow {
		color: var(--clr-text-2);
		margin: 0 -6px;
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
