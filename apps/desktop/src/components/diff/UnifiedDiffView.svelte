<script lang="ts">
	import HiddenDiffNotice from "$components/diff/HiddenDiffNotice.svelte";
	import HunkContextMenu from "$components/diff/HunkContextMenu.svelte";
	import ImageDiff from "$components/diff/ImageDiff.svelte";
	import LineLocksWarning from "$components/diff/LineLocksWarning.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import binarySvg from "$lib/assets/empty-state/binary.svg?raw";
	import emptyFileSvg from "$lib/assets/empty-state/empty-file.svg?raw";
	import tooLargeSvg from "$lib/assets/empty-state/too-large.svg?raw";
	import { DEPENDENCY_SERVICE } from "$lib/dependencies/dependencyService.svelte";
	import { draggableChips } from "$lib/dragging/draggable";
	import { HunkDropDataV3 } from "$lib/dragging/draggables";
	import { DROPZONE_REGISTRY } from "$lib/dragging/registry";
	import {
		canBePartiallySelected,
		getLineLocks,
		hunkHeaderEquals,
		type DiffHunk,
	} from "$lib/hunks/hunk";
	import { IRC_API_SERVICE } from "$lib/irc/ircApiService";
	import { type SelectionId } from "$lib/selection/key";
	import { UNCOMMITTED_SERVICE } from "$lib/selection/uncommittedService.svelte";
	import { SETTINGS } from "$lib/settings/userSettings";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import { isImageFile } from "@gitbutler/shared/utils/file";
	import { EmptyStatePlaceholder, generateHunkId, HunkDiff, TestId } from "@gitbutler/ui";
	import { DRAG_STATE_SERVICE } from "@gitbutler/ui/drag/dragStateService.svelte";
	import { parseHunk } from "@gitbutler/ui/utils/diffParsing";
	import { untrack } from "svelte";
	import type { TreeChange } from "$lib/hunks/change";
	import type { FileDependencies } from "$lib/hunks/dependencies";
	import type { UnifiedDiff } from "$lib/hunks/diff";
	import type { Reaction } from "$lib/irc/ircEndpoints";
	import type { LineId } from "@gitbutler/ui/utils/diffParsing";

	const LARGE_DIFF_THRESHOLD = 1000;
	const INITIAL_HUNKS = 5;
	const HUNKS_PER_FRAME = 10;

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
		topPadding,
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
		exclusiveAction?.type === "commit" && selectionId.type === "worktree",
	);

	const isUncommittedChange = $derived(selectionId.type === "worktree");

	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const dependencyService = inject(DEPENDENCY_SERVICE);

	const fileDependenciesQuery = $derived(
		selectionId.type === "worktree"
			? dependencyService.fileDependencies(projectId, change.path)
			: undefined,
	);

	const userSettings = inject(SETTINGS);

	const assignments = $derived(uncommittedService.assignmentsByPath(stackId || null, change.path));

	const ircApiService = inject(IRC_API_SERVICE);
	const fileReactionsQuery = $derived(
		ircApiService.fileMessageReactions({ filePath: change.path }),
	);
	const fileReactions = $derived(fileReactionsQuery?.response ?? {});

	function hunkKey(hunk: DiffHunk): string {
		return `${hunk.oldStart}:${hunk.oldLines}:${hunk.newStart}:${hunk.newLines}`;
	}

	function groupReactions(
		reactions: Reaction[],
	): { emoji: string; count: number; senders: string[] }[] {
		const map = new Map<string, string[]>();
		for (const r of reactions) {
			const senders = map.get(r.reaction) ?? [];
			senders.push(r.sender);
			map.set(r.reaction, senders);
		}
		return Array.from(map.entries()).map(([emoji, senders]) => ({
			emoji,
			count: senders.length,
			senders,
		}));
	}

	function filter(hunks: DiffHunk[]): DiffHunk[] {
		if (selectionId.type !== "worktree") return hunks;
		// TODO: It does concern me that this is an N+1;
		// We could have an encoding for hunk-headers that we can then put into
		// a hash set.
		const filtered = hunks.filter((hunk) => {
			return assignments.current.some((assignment) =>
				assignment?.hunkHeader === null ? true : hunkHeaderEquals(hunk, assignment.hunkHeader),
			);
		});
		return filtered;
	}

	const filteredHunks = $derived(diff?.type === "Patch" ? filter(diff.subject.hunks) : []);
	let renderedHunkCount = $state(INITIAL_HUNKS);

	$effect(() => {
		// Reset and stream hunks progressively whenever file/diff/showAnyways changes.
		// Avoids blocking the main thread by mounting all hunk components at once.
		void change.path;
		void diff;
		void showAnyways;

		const total = untrack(() => filteredHunks.length);
		renderedHunkCount = INITIAL_HUNKS;

		if (total <= INITIAL_HUNKS) return;

		let rafId: number;
		function addMore() {
			renderedHunkCount = Math.min(renderedHunkCount + HUNKS_PER_FRAME, total);
			if (renderedHunkCount < total) {
				rafId = requestAnimationFrame(addMore);
			}
		}
		rafId = requestAnimationFrame(addMore);
		return () => cancelAnimationFrame(rafId);
	});

	function linesInclude(
		newStart: number | undefined,
		oldStart: number | undefined,
		selected: boolean,
		lines: LineId[],
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
				oldLine: line.beforeLineNumber,
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
							selectedLine.newLine === line.newLine && selectedLine.oldLine === line.oldLine,
					),
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
		{#if $userSettings.svgAsImage && change.path.toLowerCase().endsWith(".svg")}
			<ImageDiff {projectId} {change} {commitId} />
		{:else if diff === null}
			<div class="hunk-placehoder">
				<EmptyStatePlaceholder image={binarySvg} gap={12} topBottomPadding={34}>
					{#snippet caption()}
						Was not able to load the diff
					{/snippet}
				</EmptyStatePlaceholder>
			</div>
		{:else if diff.type === "Patch"}
			{@const linesModified = diff.subject.linesAdded + diff.subject.linesRemoved}
			{#if linesModified > LARGE_DIFF_THRESHOLD && !showAnyways}
				<HiddenDiffNotice
					handleShow={() => {
						showAnyways = true;
					}}
				/>
			{:else}
				{#each filteredHunks.slice(0, renderedHunkCount) as hunk, hunkIndex}
					{@const selection = uncommittedService.hunkCheckStatus(stackId, change.path, hunk)}
					{@const [_, lineLocks] = getLineLocks(hunk, fileDependencies?.dependencies ?? [])}
					{@const hunkId = generateHunkId(change.path, hunkIndex)}
					{@const reactions = fileReactions[hunkKey(hunk)] ?? []}
					<div
						class="hunk-content"
						use:draggableChips={{
							label: hunk.diff.split("\n")[0],
							data: new HunkDropDataV3(
								change,
								hunk,
								isUncommittedChange,
								stackId || null,
								commitId,
								selectionId,
							),
							disabled: !draggable,
							chipType: "hunk",
							dropzoneRegistry,
							dragStateService,
						}}
					>
						<HunkDiff
							id={hunkId}
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
							strongContrast={$userSettings.strongContrast}
							colorBlindFriendly={$userSettings.colorBlindFriendly}
							inlineUnifiedDiffs={$userSettings.inlineUnifiedDiffs}
							selectable={isUncommittedChange}
							onLineClick={(p) => {
								if (!canBePartiallySelected(diff.subject)) {
									uncommittedService.checkHunk(stackId || null, change.path, hunk);
								}
								if (
									!linesInclude(
										p.newLine,
										p.oldLine,
										selection.current.selected,
										selection.current.lines,
									)
								) {
									uncommittedService.checkLine(stackId || null, change.path, hunk, {
										newLine: p.newLine,
										oldLine: p.oldLine,
									});
								} else {
									const allLines =
										p.rows
											?.filter((l) => l.isDeltaLine)
											.map((l) => ({
												newLine: l.afterLineNumber,
												oldLine: l.beforeLineNumber,
											})) ?? [];
									uncommittedService.uncheckLine(
										stackId || null,
										change.path,
										hunk,
										{
											newLine: p.newLine,
											oldLine: p.oldLine,
										},
										allLines,
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
									afterLineNumber: params.afterLineNumber,
								});
							}}
						>
							{#snippet lockWarning(locks)}
								<LineLocksWarning {projectId} {locks} />
							{/snippet}
						</HunkDiff>
						{#if reactions.length > 0}
							<div class="hunk-reactions">
								{#each groupReactions(reactions) as group}
									<span class="hunk-reaction-pill" title={group.senders.join(", ")}>
										{group.emoji}
										{#if group.count > 1}
											{group.count}
										{/if}
									</span>
								{/each}
							</div>
						{/if}
					</div>
				{:else}
					{#if diff.subject.hunks.length === 0}
						<div class="hunk-placehoder">
							<EmptyStatePlaceholder image={emptyFileSvg} gap={12} topBottomPadding={34}>
								{#snippet caption()}
									It’s empty ¯\_(ツ゚)_/¯
								{/snippet}
							</EmptyStatePlaceholder>
						</div>
					{:else}
						<div class="hunk-placehoder">
							<EmptyStatePlaceholder gap={12} topBottomPadding={34}>
								{#snippet caption()}
									Loading diff…
								{/snippet}
							</EmptyStatePlaceholder>
						</div>
					{/if}
				{/each}
			{/if}
		{:else if diff.type === "TooLarge"}
			<div class="hunk-placehoder">
				<EmptyStatePlaceholder image={tooLargeSvg} gap={12} topBottomPadding={34}>
					{#snippet caption()}
						Too large to display
					{/snippet}
				</EmptyStatePlaceholder>
			</div>
		{:else if diff.type === "Binary"}
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
		<!-- The context menu should be outside the each block. -->
		<HunkContextMenu
			bind:this={contextMenu}
			trigger={viewport}
			{projectId}
			{change}
			{stackId}
			{commitId}
			discardable={isUncommittedChange}
			{selectable}
			{selectAllHunkLines}
			{unselectAllHunkLines}
			{invertHunkSelection}
		/>
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
		border: 1px solid var(--border-3);
		border-radius: var(--radius-m);
	}

	.hunk-content {
		user-select: text;
	}
	.hunk-reactions {
		display: flex;
		align-items: center;
		padding: 4px 0 0;
		gap: 4px;
	}
	.hunk-reaction-pill {
		display: inline-flex;
		align-items: center;
		padding: 2px 6px;
		gap: 4px;
		border: 1px solid transparent;
		border-radius: 10px;
		background-color: var(--bg-2);
		font-size: 12px;
	}
</style>
