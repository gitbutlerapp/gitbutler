<script lang="ts">
	import LargeDiffMessage from '$components/LargeDiffMessage.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import HunkContextMenu from '$components/v3/HunkContextMenu.svelte';
	import LineLocksWarning from '$components/v3/LineLocksWarning.svelte';
	import binarySvg from '$lib/assets/empty-state/binary.svg?raw';
	import emptyFileSvg from '$lib/assets/empty-state/empty-file.svg?raw';
	import tooLargeSvg from '$lib/assets/empty-state/too-large.svg?raw';
	import DependencyService from '$lib/dependencies/dependencyService.svelte';
	import { DragStateService } from '$lib/dragging/dragStateService.svelte';
	import { draggableChips } from '$lib/dragging/draggable';
	import { HunkDropDataV3 } from '$lib/dragging/draggables';
	import { DropzoneRegistry } from '$lib/dragging/registry';
	import {
		canBePartiallySelected,
		getLineLocks,
		hunkHeaderEquals,
		type DiffHunk
	} from '$lib/hunks/hunk';
	import { Project } from '$lib/project/project';
	import { type SelectionId } from '$lib/selection/key';
	import { UncommittedService } from '$lib/selection/uncommittedService.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { getContextStoreBySymbol, inject } from '@gitbutler/shared/context';
	import EmptyStatePlaceholder from '@gitbutler/ui/EmptyStatePlaceholder.svelte';
	import HunkDiff from '@gitbutler/ui/HunkDiff.svelte';
	import type { TreeChange } from '$lib/hunks/change';
	import type { UnifiedDiff } from '$lib/hunks/diff';
	import type { LineId } from '@gitbutler/ui/utils/diffParsing';

	const LARGE_DIFF_THRESHOLD = 2500;

	type Props = {
		projectId: string;
		selectable: boolean;
		change: TreeChange;
		diff: UnifiedDiff;
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

	const [project, uiState, dropzoneRegistry, dragStateService] = inject(
		Project,
		UiState,
		DropzoneRegistry,
		DragStateService
	);

	let contextMenu = $state<ReturnType<typeof HunkContextMenu>>();
	let showAnyways = $state(false);
	let viewport = $state<HTMLDivElement>();

	const projectState = $derived(uiState.project(projectId));
	const exclusiveAction = $derived(projectState.exclusiveAction.current);

	const isCommiting = $derived(
		exclusiveAction?.type === 'commit' && selectionId.type === 'worktree'
	);

	const isUncommittedChange = $derived(selectionId.type === 'worktree');

	const [uncommittedService, dependencyService] = inject(UncommittedService, DependencyService);

	const fileDependenciesResult = $derived(
		dependencyService.fileDependencies(projectId, change.path)
	);

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);

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
</script>

<ReduxResult {projectId} result={fileDependenciesResult.current}>
	{#snippet children(fileDependencies)}
		<div
			data-testid={TestId.UnifiedDiffView}
			class="diff-section"
			class:top-padding={topPadding}
			bind:this={viewport}
		>
			{#if diff.type === 'Patch'}
				{@const linesModified = diff.subject.linesAdded + diff.subject.linesRemoved}
				{#if linesModified > LARGE_DIFF_THRESHOLD && !showAnyways}
					<LargeDiffMessage
						handleShow={() => {
							showAnyways = true;
						}}
					/>
				{:else}
					{#each filter(diff.subject.hunks) as hunk}
						{@const selection = uncommittedService.hunkCheckStatus(
							stackId || null,
							change.path,
							hunk
						)}
						{@const [_, lineLocks] = getLineLocks(hunk, fileDependencies.dependencies ?? [])}
						<div
							class="hunk-content"
							use:draggableChips={{
								label: hunk.diff.split('\n')[0],
								data: new HunkDropDataV3(
									change,
									hunk,
									isUncommittedChange,
									stackId || null,
									commitId,
									selectionId
								),
								disabled: !draggable,
								chipType: 'hunk',
								dropzoneRegistry,
								dragStateService
							}}
						>
							<HunkDiff
								draggingDisabled={!draggable}
								hideCheckboxes={!isCommiting}
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
									contextMenu?.open(params.event, {
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
					projectPath={project.vscodePath}
					{projectId}
					{change}
					discardable={isUncommittedChange}
					unSelectHunk={(hunk) => {
						uncommittedService.uncheckHunk(stackId || null, change.path, hunk);
					}}
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
				<div class="hunk-placehoder">
					<EmptyStatePlaceholder image={binarySvg} gap={12} topBottomPadding={34}>
						{#snippet caption()}
							Binary! Not for human eyes
						{/snippet}
					</EmptyStatePlaceholder>
				</div>
			{/if}
		</div>
	{/snippet}
</ReduxResult>

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
