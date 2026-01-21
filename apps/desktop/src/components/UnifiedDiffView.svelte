<script lang="ts">
	import HunkContextMenu from '$components/HunkContextMenu.svelte';
	import ImageDiff from '$components/ImageDiff.svelte';
	import LargeDiffMessage from '$components/LargeDiffMessage.svelte';
	import LineLocksWarning from '$components/LineLocksWarning.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import binarySvg from '$lib/assets/empty-state/binary.svg?raw';
	import emptyFileSvg from '$lib/assets/empty-state/empty-file.svg?raw';
	import tooLargeSvg from '$lib/assets/empty-state/too-large.svg?raw';
	import { DEPENDENCY_SERVICE } from '$lib/dependencies/dependencyService.svelte';
	import { draggableChips } from '$lib/dragging/draggable';
	import { HunkDropDataV3 } from '$lib/dragging/draggables';
	import { DROPZONE_REGISTRY } from '$lib/dragging/registry';
	import {
		canBePartiallySelected,
		getLineLocks,
		hunkHeaderEquals,
		lineIdsToHunkHeaders,
		type DiffHunk
	} from '$lib/hunks/hunk';
	import { type SelectionId } from '$lib/selection/key';
	import { UNCOMMITTED_SERVICE } from '$lib/selection/uncommittedService.svelte';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/core/context';
	import { isImageFile } from '@gitbutler/shared/utils/file';
	import { EmptyStatePlaceholder, HunkDiff, TestId } from '@gitbutler/ui';
	import { DRAG_STATE_SERVICE } from '@gitbutler/ui/drag/dragStateService.svelte';
	import { parseHunk } from '@gitbutler/ui/utils/diffParsing';
	import type { FileDependencies } from '$lib/dependencies/dependencies';
	import type { TreeChange } from '$lib/hunks/change';
	import type { UnifiedDiff } from '$lib/hunks/diff';
	import type { LineId } from '@gitbutler/ui/utils/diffParsing';

	const LARGE_DIFF_THRESHOLD = 1000;

	type Props = {
		projectId: string;
		selectable: boolean;
		change: TreeChange;
		diff: UnifiedDiff | null;
		selectionId: SelectionId;
		stackId?: string;
		commitId?: string;
		draggable?: boolean;
		topPadding?: boolean;
	};

	const {
		projectId,
		selectable = false,
		change,
		diff,
		selectionId,
		stackId,
		commitId,
		draggable,
		topPadding
	}: Props = $props();

	const uiState = inject(UI_STATE);
	const dropzoneRegistry = inject(DROPZONE_REGISTRY);
	const dragStateService = inject(DRAG_STATE_SERVICE);

	let contextMenu = $state<ReturnType<typeof HunkContextMenu>>();
	let showAnyways = $state(false);
	let viewport = $state<HTMLDivElement>();

	const projectState = $derived(uiState.project(projectId));
	const exclusiveAction = $derived(projectState.exclusiveAction.current);

	const isCommitting = $derived(
		exclusiveAction?.type === 'commit' && selectionId.type === 'worktree'
	);

	const isUncommittedChange = $derived(selectionId.type === 'worktree');

	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const dependencyService = inject(DEPENDENCY_SERVICE);

	const fileDependenciesQuery = $derived(
		selectionId.type === 'worktree'
			? dependencyService.fileDependencies(projectId, change.path)
			: undefined
	);

	const userSettings = inject(SETTINGS);

	const assignments = $derived(uncommittedService.assignmentsByPath(stackId || null, change.path));

	function filter(hunks: DiffHunk[]): DiffHunk[] {
		if (selectionId.type !== 'worktree') return hunks;
		// TODO: It does concern me that this is an N+1;
		// We could have an encoding for hunk-headers that we can then put into
		// a hash set.
		const filtered = hunks.filter((hunk) => {
			return assignments.current.some((assignment) =>
				assignment?.hunkHeader === null ? true : hunkHeaderEquals(hunk, assignment.hunkHeader)
			);
		});
		return filtered;
	}

	function linesInclude(
		newStart: number | undefined,
		oldStart: number | undefined,
		selected: boolean,
		lines: LineId[]
	) {
		if (!selected) return false;
		return (
			lines.length === 0 || lines.some((l) => l.newLine === newStart && l.oldLine === oldStart)
		);
	}

	function selectAllHunkLines(hunk: DiffHunk) {
		uncommittedService.checkHunk(stackId || null, change.path, hunk);
	}

	function unselectAllHunkLines(hunk: DiffHunk) {
		uncommittedService.uncheckHunk(stackId || null, change.path, hunk);
	}

	function invertHunkSelection(hunk: DiffHunk) {
		// Parse the hunk to get all selectable lines
		const parsedHunk = parseHunk(hunk.diff);
		const allSelectableLines = parsedHunk.contentSections
			.flatMap((section) => section.lines)
			.filter((line) => line.beforeLineNumber !== undefined || line.afterLineNumber !== undefined)
			.map((line) => ({
				newLine: line.afterLineNumber,
				oldLine: line.beforeLineNumber
			}));

		const selection = uncommittedService.hunkCheckStatus(stackId, change.path, hunk);
		const currentSelectedLines = selection.current.lines;
		const isSelected = selection.current.selected;

		// If nothing is selected (hunk not checked)
		if (!isSelected) {
			selectAllHunkLines(hunk);
		}
		// If all lines are selected (empty lines array indicates full selection)
		else if (isSelected && currentSelectedLines.length === 0) {
			unselectAllHunkLines(hunk);
		} else {
			const unselectedLines = allSelectableLines.filter(
				(line) =>
					!currentSelectedLines.some(
						(selectedLine) =>
							selectedLine.newLine === line.newLine && selectedLine.oldLine === line.oldLine
					)
			);

			// First unselect all lines
			unselectAllHunkLines(hunk);

			// Then select the previously unselected lines
			unselectedLines.forEach((line) => {
				uncommittedService.checkLine(stackId || null, change.path, hunk, line);
			});
		}
	}
</script>

{#if fileDependenciesQuery}
	<ReduxResult {projectId} result={fileDependenciesQuery.result} children={unifiedDiff} />
{:else}
	{@render unifiedDiff(undefined)}
{/if}

{#snippet unifiedDiff(fileDependencies: FileDependencies | undefined)}
	<div
		data-testid={TestId.UnifiedDiffView}
		class="diff-section"
		class:top-padding={topPadding}
		bind:this={viewport}
	>
		{#if diff === null}
			<div class="hunk-placehoder">
				<EmptyStatePlaceholder image={binarySvg} gap={12} topBottomPadding={34}>
					{#snippet caption()}
						Was not able to load the diff
					{/snippet}
				</EmptyStatePlaceholder>
			</div>
		{:else if diff.type === 'Patch'}
			{@const linesModified = diff.subject.linesAdded + diff.subject.linesRemoved}
			{#if linesModified > LARGE_DIFF_THRESHOLD && !showAnyways}
				<LargeDiffMessage
					handleShow={() => {
						showAnyways = true;
					}}
				/>
			{:else}
				{#each filter(diff.subject.hunks) as hunk}
					{@const selection = uncommittedService.hunkCheckStatus(stackId, change.path, hunk)}
					{@const selectedHunkHeaders =
						selection.current.selected && selection.current.lines.length > 0
							? lineIdsToHunkHeaders(
									selection.current.lines,
									hunk.diff,
									selectionId.type === 'worktree' ? 'commit' : 'discard'
								)
							: undefined}
					{@const [_, lineLocks] = getLineLocks(hunk, fileDependencies?.dependencies ?? [])}
					<div
						class="hunk-content"
						use:draggableChips={{
								label: selectedHunkHeaders ? `${hunk.diff.split('\n')[0]} (${selection.current.lines.length} selected lines)` : hunk.diff.split('\n')[0],
							data: new HunkDropDataV3(
								change,
								hunk,
								isUncommittedChange,
								stackId || null,
								commitId,
								selectionId,
								selectedHunkHeaders
							),
							disabled: !draggable,
							chipType: 'hunk',
							dropzoneRegistry,
							dragStateService
						}}
					>
						<HunkDiff
							draggingDisabled={!draggable}
							hideCheckboxes={!isCommitting}
							filePath={change.path}
							hunkStr={hunk.diff}
							staged={selection.current.selected}
							stagedLines={selection.current.lines}
							{lineLocks}
							diffLigatures={$userSettings.diffLigatures}
							tabSize={$userSettings.tabSize}
							wrapText={$userSettings.wrapText}
							diffFont={$userSettings.diffFont}
							diffContrast={$userSettings.diffContrast}
							colorBlindFriendly={$userSettings.colorBlindFriendly}
							inlineUnifiedDiffs={$userSettings.inlineUnifiedDiffs}
							onLineClick={(p) => {
								if (!canBePartiallySelected(diff.subject)) {
									uncommittedService.checkHunk(stackId || null, change.path, hunk);
								}
								if (
									!linesInclude(
										p.newLine,
										p.oldLine,
										selection.current.selected,
										selection.current.lines
									)
								) {
									uncommittedService.checkLine(stackId || null, change.path, hunk, {
										newLine: p.newLine,
										oldLine: p.oldLine
									});
								} else {
									const allLines =
										p.rows
											?.filter((l) => l.isDeltaLine)
											.map((l) => ({
												newLine: l.afterLineNumber,
												oldLine: l.beforeLineNumber
											})) ?? [];
									uncommittedService.uncheckLine(
										stackId || null,
										change.path,
										hunk,
										{
											newLine: p.newLine,
											oldLine: p.oldLine
										},
										allLines
									);
								}
							}}
							onChangeStage={(selected) => {
								if (!selectable) return;
								if (selected) {
									uncommittedService.checkHunk(stackId || null, change.path, hunk);
								} else {
									uncommittedService.uncheckHunk(stackId || null, change.path, hunk);
								}
							}}
							handleLineContextMenu={(params) => {
								contextMenu?.open(params.event || params.target, {
									hunk,
									selectedLines: selection.current.lines,
									beforeLineNumber: params.beforeLineNumber,
									afterLineNumber: params.afterLineNumber
								});
							}}
						>
							{#snippet lockWarning(locks)}
								<LineLocksWarning {projectId} {locks} />
							{/snippet}
						</HunkDiff>
					</div>
				{:else}
					<div class="hunk-placehoder">
						<EmptyStatePlaceholder image={emptyFileSvg} gap={12} topBottomPadding={34}>
							{#snippet caption()}
								It’s empty ¯\_(ツ゚)_/¯
							{/snippet}
						</EmptyStatePlaceholder>
					</div>
				{/each}
			{/if}

			<!-- The context menu should be outside the each block. -->
			<HunkContextMenu
				bind:this={contextMenu}
				trigger={viewport}
				{projectId}
				{change}
				discardable={isUncommittedChange}
				{selectable}
				{selectAllHunkLines}
				{unselectAllHunkLines}
				{invertHunkSelection}
			/>
		{:else if diff.type === 'TooLarge'}
			<div class="hunk-placehoder">
				<EmptyStatePlaceholder image={tooLargeSvg} gap={12} topBottomPadding={34}>
					{#snippet caption()}
						Too large to display
					{/snippet}
				</EmptyStatePlaceholder>
			</div>
		{:else if diff.type === 'Binary'}
			{#if isImageFile(change.path)}
				<ImageDiff {projectId} {change} {commitId} />
			{:else}
				<div class="hunk-placehoder">
					<EmptyStatePlaceholder image={binarySvg} gap={12} topBottomPadding={34}>
						{#snippet caption()}
							Binary! Not for human eyes
						{/snippet}
					</EmptyStatePlaceholder>
				</div>
			{/if}
		{/if}
	</div>
{/snippet}

<style lang="postcss">
	.diff-section {
		display: flex;
		flex-direction: column;
		align-self: stretch;
		max-width: 100%;
		padding: 0 14px 14px 14px;
		overflow-x: hidden;
		gap: 14px;
		&.top-padding {
			padding-top: 14px;
		}
	}
	.hunk-placehoder {
		border: 1px solid var(--clr-border-3);
		border-radius: var(--radius-m);
	}

	.hunk-content {
		user-select: text;
	}
</style>
