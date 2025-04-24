<script lang="ts">
	import HunkContextMenu from '$components/v3/HunkContextMenu.svelte';
	import LineSelection from '$components/v3/unifiedDiffLineSelection.svelte';
	import binarySvg from '$lib/assets/empty-state/binary.svg?raw';
	import emptyFileSvg from '$lib/assets/empty-state/empty-file.svg?raw';
	import tooLargeSvg from '$lib/assets/empty-state/too-large.svg?raw';
	import { draggableChips } from '$lib/dragging/draggable';
	import { ChangeDropData } from '$lib/dragging/draggables';
	import { canBePartiallySelected, type DiffHunk } from '$lib/hunks/hunk';
	import { Project } from '$lib/project/project';
	import {
		ChangeSelectionService,
		type PartiallySelectedFile
	} from '$lib/selection/changeSelection.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { type SelectionId } from '$lib/selection/key';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContextStoreBySymbol, inject } from '@gitbutler/shared/context';
	import EmptyStatePlaceholder from '@gitbutler/ui/EmptyStatePlaceholder.svelte';
	import HunkDiff from '@gitbutler/ui/HunkDiff.svelte';
	import type { TreeChange } from '$lib/hunks/change';
	import type { UnifiedDiff } from '$lib/hunks/diff';
	import type { LineId } from '@gitbutler/ui/utils/diffParsing';

	type Props = {
		projectId: string;
		selectable: boolean;
		change: TreeChange;
		diff: UnifiedDiff;
		selectionId: SelectionId;
	};

	const { projectId, selectable = false, change, diff, selectionId }: Props = $props();
	const [project, uiState] = inject(Project, UiState);
	let contextMenu = $state<ReturnType<typeof HunkContextMenu>>();
	let viewport = $state<HTMLDivElement>();
	const projectState = $derived(uiState.project(projectId));
	const drawerPage = $derived(projectState.drawerPage.current);
	const isCommiting = $derived(drawerPage === 'new-commit');
	const readonly = $derived(selectionId.type !== 'worktree');

	const [changeSelection, idSelection, lineSelection] = inject(
		ChangeSelectionService,
		IdSelection,
		LineSelection
	);

	const changeSelectionResult = $derived(changeSelection.getById(change.path));
	const selection = $derived(changeSelectionResult.current);
	const pathData = $derived({
		path: change.path,
		pathBytes: change.pathBytes
	});

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);

	$effect(() => {
		lineSelection.setChange(change);
	});

	$effect(() => {
		lineSelection.setSelectable(selectable);
	});

	function updateStage(hunk: DiffHunk, select: boolean, allHunks: DiffHunk[]) {
		if (selection?.type === 'full') {
			handleStageInFullSelection(select, allHunks, hunk);
			return;
		}

		if (selection?.type === 'partial') {
			handleStageInPartialSelection(selection, select, hunk, allHunks);
			return;
		}

		if (select) {
			changeSelection.add({
				type: 'partial',
				...pathData,
				hunks: [{ type: 'full', ...hunk }]
			});
			return;
		}
	}

	function unselectHunk(hunk: DiffHunk, allHunks: DiffHunk[]) {
		updateStage(hunk, false, allHunks);
		if (allHunks.length === 1) {
			// This is the only hunk, so we can unselect the file
			idSelection.remove(change.path, selectionId);
		}
	}

	/**
	 * Handles updating the staging state of a hunk when the file it belongs to is already partially staged.
	 */
	function handleStageInPartialSelection(
		partialSelection: PartiallySelectedFile,
		select: boolean,
		hunk: DiffHunk,
		allHunks: DiffHunk[]
	) {
		const newHunks = partialSelection.hunks.slice();

		if (select) {
			newHunks.push({
				type: 'full',
				...hunk
			});

			if (newHunks.length === allHunks.length) {
				changeSelection.update({
					type: 'full',
					...pathData
				});
				return;
			}

			changeSelection.update({
				type: 'partial',
				...pathData,
				hunks: newHunks
			});

			return;
		}

		const hunks = partialSelection.hunks.filter((h) => {
			return h.newStart !== hunk.newStart && h.newLines !== hunk.newLines;
		});

		if (hunks.length === 0) {
			changeSelection.remove(change.path);
		} else {
			changeSelection.update({
				type: 'partial',
				...pathData,
				hunks
			});
		}
	}

	/**
	 * Handles updating the staging state of a hunk when the file it belongs to is already fully staged.
	 */
	function handleStageInFullSelection(select: boolean, allHunks: DiffHunk[], hunk: DiffHunk) {
		if (select) {
			throw new Error('Cannot add to full selection');
		}
		const newHunks = allHunks.filter((h) => h !== hunk);
		changeSelection.update({
			...pathData,
			type: 'partial',
			hunks: newHunks.map((h) => ({
				type: 'full',
				...h
			}))
		});
	}

	function getStageState(hunk: DiffHunk): [boolean | undefined, LineId[] | undefined] {
		if (!selectable) return [undefined, undefined];
		if (selection === undefined) return [false, undefined];
		if (selection.type === 'full') return [true, undefined];
		const hunkSelected = selection.hunks.find(
			(h) => h.newStart === hunk.newStart && h.oldStart === hunk.oldStart
		);
		const linesSelected = hunkSelected?.type === 'partial' ? hunkSelected?.lines : undefined;
		const stagedHunk = !!hunkSelected;
		return [stagedHunk, linesSelected];
	}
</script>

<div class="diff-section" bind:this={viewport}>
	{#if diff.type === 'Patch'}
		{#each diff.subject.hunks as hunk}
			{@const [staged, stagedLines] = getStageState(hunk)}

			<div
				class="hunk-content no-select"
				use:draggableChips={{
					label: hunk.diff.split('\n')[0],
					data: new ChangeDropData(change, idSelection, selectionId),
					disabled: readonly,
					chipType: 'hunk'
				}}
			>
				<HunkDiff
					draggingDisabled={readonly}
					hideCheckboxes={!isCommiting}
					filePath={change.path}
					hunkStr={hunk.diff}
					{staged}
					{stagedLines}
					diffLigatures={$userSettings.diffLigatures}
					tabSize={$userSettings.tabSize}
					wrapText={$userSettings.wrapText}
					diffFont={$userSettings.diffFont}
					diffContrast={$userSettings.diffContrast}
					inlineUnifiedDiffs={$userSettings.inlineUnifiedDiffs}
					onLineClick={(p) => {
						if (!canBePartiallySelected(diff.subject)) {
							const select = selection === undefined;
							updateStage(hunk, select, diff.subject.hunks);
							return;
						}
						lineSelection.toggleStageLines(selection, hunk, p, diff.subject.hunks);
					}}
					onChangeStage={(selected) => {
						updateStage(hunk, selected, diff.subject.hunks);
					}}
					handleLineContextMenu={(params) => {
						contextMenu?.open(params.event, {
							hunk,
							selectedLines: stagedLines,
							beforeLineNumber: params.beforeLineNumber,
							afterLineNumber: params.afterLineNumber
						});
					}}
				/>
			</div>
			<HunkContextMenu
				bind:this={contextMenu}
				trigger={viewport}
				projectPath={project.vscodePath}
				{projectId}
				{change}
				{readonly}
				unSelectHunk={(hunk) => unselectHunk(hunk, diff.subject.hunks)}
			/>
		{:else}
			<div class="hunk-placehoder">
				<EmptyStatePlaceholder image={emptyFileSvg} gap={12} topBottomPadding={34}>
					{#snippet caption()}
						It’s empty ¯\_(ツ゚)_/¯
					{/snippet}
				</EmptyStatePlaceholder>
			</div>
		{/each}
	{:else if diff.type === 'TooLarge'}
		<div class="hunk-placehoder">
			<EmptyStatePlaceholder image={tooLargeSvg} gap={12} topBottomPadding={34}>
				{#snippet caption()}
					Too large to display
				{/snippet}
			</EmptyStatePlaceholder>
		</div>
	{:else if diff.type === 'Binary'}
		<div class="hunk-placehoder">
			<EmptyStatePlaceholder image={binarySvg} gap={12} topBottomPadding={34}>
				{#snippet caption()}
					Binary! Not for human eyes
				{/snippet}
			</EmptyStatePlaceholder>
		</div>
	{/if}
</div>

<style lang="postcss">
	.diff-section {
		display: flex;
		padding: 0 14px 14px 14px;
		flex-direction: column;
		gap: 14px;
		align-self: stretch;
		overflow-x: hidden;
		max-width: 100%;
	}
	.hunk-placehoder {
		border: 1px solid var(--clr-border-3);
		border-radius: var(--radius-m);
	}
</style>
