<script lang="ts">
	/**
	 * NOTE: This component MOST only ever be rendered ONCE on the page at one
	 * time. This is because it is working directly with the query paramaters
	 * and has no idea if it will conflict or not.
	 */
	import SectionComponent from '$lib/components/review/Section.svelte';
	import {
		setBeforeVersion,
		setAfterVersion,
		getBeforeVersion,
		getAfterVersion
	} from '$lib/interdiffRangeQuery.svelte';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { getPatchIdableSections } from '@gitbutler/shared/patches/patchCommitsPreview.svelte';
	import {
		Button,
		Select,
		SelectItem,
		type LineClickParams,
		type SelectItemType
	} from '@gitbutler/ui';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import type { PatchCommit } from '@gitbutler/shared/patches/types';
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

	const userService = inject(USER_SERVICE);
	const user = $derived(userService.user);

	const isLoggedIn = $derived(!!$user);

	let isInterdiffBarVisible = $state(false);

	const allOptions: readonly SelectItemType<string>[] = $derived.by(() => {
		const out = [{ value: '-1', label: 'Base' }];

		if (!isDefined(patchCommit.version)) return out;

		for (let i = 1; i <= patchCommit.version; ++i) {
			const last = i === patchCommit.version;

			out.push({
				value: i.toString(),
				label: `v${i}${last ? ' (latest)' : ''}`
			});
		}

		return out;
	});

	const beforeOptions = $derived(allOptions.slice(0, -1));
	const afterOptions = $derived(allOptions.slice(1));

	const beforeVersionResult = getBeforeVersion();
	const selectedVersionResult = $derived(getAfterVersion(patchCommit.version));

	const selectedBefore = $derived(beforeVersionResult.current);
	const selectedAfter = $derived(selectedVersionResult.current);

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

	const interdiffActive = $derived(selectedBefore !== -1 || selectedAfter !== patchCommit.version);

	$effect(() => {
		// If the user starts to view an interdiff range, open the interdiff bar
		if (interdiffActive) {
			isInterdiffBarVisible = true;
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
					{#if interdiffActive}
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
						setBeforeVersion(parseInt(value));
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
							disabled={!isSelected && (item.value ? parseInt(item.value) >= selectedAfter : false)}
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
						setAfterVersion(patchCommit.version, parseInt(value));
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
							disabled={!isSelected &&
								(item.value ? parseInt(item.value) <= selectedBefore : false)}
						>
							{item.label}
						</SelectItem>
					{/snippet}
				</Select>

				{#if interdiffActive}
					<Button
						kind="ghost"
						icon="undo-small"
						size="tag"
						tooltip="Reset to initial selection"
						onclick={async () => {
							await setBeforeVersion(-1);
							await setAfterVersion(patchCommit.version, patchCommit.version);
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
		contain: paint;
		display: flex;
		flex-direction: column;
	}

	.review-sections-statistics-wrap {
		display: flex;
		width: 100%;
	}

	.review-sections-statistics {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding: 10px 10px 10px 14px;
		border: 1px solid var(--clr-border-2);
		border-top-right-radius: var(--radius-ml);
		border-top-left-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
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
		display: flex;
		position: relative;
		flex-direction: column;
		width: 100%;
	}

	/* INTERDIFF */

	.interdiff-bar {
		display: flex;
		align-items: center;
		width: 100%;

		padding: 14px;
		gap: 12px;

		border: 1px solid var(--clr-border-2);
		border-top: none;

		background-color: var(--clr-bg-1-muted);

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
		display: flex;
		position: relative;
	}

	.review-sections-statistics__actions__interdiff-changed {
		position: absolute;
		top: 2px;
		right: 2px;
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background-color: var(--clr-theme-pop-element);
	}
</style>
