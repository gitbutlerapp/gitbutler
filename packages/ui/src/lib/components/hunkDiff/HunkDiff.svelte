<script lang="ts" module>
	import { type LineSelectionParams } from "$components/hunkDiff/lineSelection.svelte";
	export type LineClickParams = LineSelectionParams;
</script>

<script lang="ts">
	import Icon from "$components/Icon.svelte";
	import HunkDiffBody from "$components/hunkDiff/HunkDiffBody.svelte";
	import ScrollableContainer from "$components/scroll/ScrollableContainer.svelte";
	import { focusable } from "$lib/focus/focusable";
	import {
		type DependencyLock,
		type LineId,
		type LineLock,
		parseHunk,
	} from "$lib/utils/diffParsing";
	import type { ContextMenuParams } from "$components/hunkDiff/HunkDiffRow.svelte";
	import type { Snippet } from "svelte";

	interface Props {
		id?: string;
		filePath: string;
		hunkStr: string;
		tabSize?: number;
		wrapText?: boolean;
		diffFont?: string;
		diffLigatures?: boolean;
		inlineUnifiedDiffs?: boolean;
		strongContrast?: boolean;
		colorBlindFriendly?: boolean;
		staged?: boolean;
		stagedLines?: LineId[];
		hideCheckboxes?: boolean;
		selectable?: boolean;
		lineLocks?: LineLock[];
		draggingDisabled?: boolean;
		onChangeStage?: (staged: boolean) => void;
		onLineClick?: (params: LineSelectionParams) => void;
		handleLineContextMenu?: (params: ContextMenuParams) => void;
		lockWarning?: Snippet<[DependencyLock[]]>;
	}

	const {
		id,
		filePath,
		hunkStr,
		tabSize = 4,
		wrapText = true,
		diffFont = "var(--font-mono)",
		diffLigatures = true,
		strongContrast = false,
		colorBlindFriendly = false,
		inlineUnifiedDiffs = false,
		staged,
		stagedLines,
		selectable,
		hideCheckboxes,
		lineLocks,
		onChangeStage,
		onLineClick,
		handleLineContextMenu,
		draggingDisabled,
		lockWarning,
	}: Props = $props();

	const BORDER_WIDTH = 1;

	let numberHeaderWidth = $state<number>(0);

	const hunk = $derived(parseHunk(hunkStr));

	const hunkSummary = $derived(
		`@@ -${hunk.oldStart},${hunk.oldLines} +${hunk.newStart},${hunk.newLines} @@`,
	);
	const showingCheckboxes = $derived(!hideCheckboxes && staged !== undefined);
	const hunkHasLocks = $derived(lineLocks && lineLocks.length > 0);
	const colspan = $derived(showingCheckboxes || hunkHasLocks ? 3 : 2);
	let tableWrapperElem = $state<HTMLElement>();

	function handleHunkContextMenu(e: MouseEvent | KeyboardEvent) {
		e.preventDefault();
		e.stopPropagation();

		if (handleLineContextMenu) {
			handleLineContextMenu({
				event: e instanceof MouseEvent ? e : undefined,
				target: tableWrapperElem,
				beforeLineNumber: undefined,
				afterLineNumber: undefined,
			});
		}
	}
</script>

<div
	{id}
	use:focusable={{
		onKeydown: (e) => {
			if (e.key === "Control") {
				handleHunkContextMenu(e);
			}
		},
	}}
	bind:this={tableWrapperElem}
	class="table__wrapper"
	class:contrast-strong={strongContrast}
	class:colorblind-friendly={colorBlindFriendly}
	style="--tab-size: {tabSize}; --diff-font: {diffFont};"
	style:font-variant-ligatures={diffLigatures ? "common-ligatures" : "none"}
>
	{#if !draggingDisabled}
		<div class="table__drag-handle">
			<Icon name="draggable" />
		</div>
	{/if}
	<ScrollableContainer horz whenToShow="always" zIndex="0">
		<table class="table__section">
			<thead class="table__title" class:draggable={!draggingDisabled}>
				<tr>
					<th
						id={id ? `header-${id}` : undefined}
						bind:clientWidth={numberHeaderWidth}
						class="table__checkbox-container"
						style="--border-width: {BORDER_WIDTH}px;"
						class:stageable={showingCheckboxes}
						class:staged={showingCheckboxes && staged}
						{colspan}
						onclick={() => {
							if (showingCheckboxes) {
								onChangeStage?.(!staged);
							}
						}}
						oncontextmenu={handleHunkContextMenu}
					>
						<div class="table__checkbox" class:staged>
							{#if staged && !hideCheckboxes}
								<Icon name="tick-small" />
							{:else if showingCheckboxes}
								<Icon name="minus-small" />
							{/if}
						</div>
					</th>

					<th class="table__title-content" {colspan} oncontextmenu={handleHunkContextMenu}>
						<span>
							{hunkSummary}
						</span>
					</th>
				</tr>
			</thead>

			{#if tableWrapperElem}
				<HunkDiffBody
					comment={hunk.comment}
					{filePath}
					{selectable}
					content={hunk.contentSections}
					{onLineClick}
					{wrapText}
					{tabSize}
					{diffFont}
					{inlineUnifiedDiffs}
					{lineLocks}
					{numberHeaderWidth}
					{staged}
					{stagedLines}
					{hideCheckboxes}
					{handleLineContextMenu}
					{lockWarning}
				/>
			{/if}
		</table>
	</ScrollableContainer>
</div>

<style lang="postcss">
	.table__wrapper {
		position: relative;
		width: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-diff-count-border);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);

		&:hover .table__drag-handle {
			opacity: 1;
		}
	}

	table,
	.table__section {
		width: 100%;
		min-width: 0;
		border-collapse: separate;
		border-spacing: 0;
		font-family: var(--diff-font);
	}

	thead {
		width: 100%;
		padding: 0;
	}

	th,
	tr {
		margin: 0;
		padding: 0;
	}

	table thead th {
		height: 28px;
	}

	.table__checkbox-container {
		box-sizing: border-box;
		border-right: 1px solid var(--clr-diff-count-border);
		border-bottom: 1px solid var(--clr-diff-count-border);
		background-color: var(--clr-diff-count-bg);

		&.stageable {
			cursor: pointer;
		}

		&.staged {
			border-color: var(--clr-diff-selected-count-border);
			background-color: var(--clr-diff-selected-count-bg);
		}
	}

	.table__checkbox {
		display: flex;
		align-items: center;
		justify-content: flex-start;
		padding: 4px;
		color: var(--clr-diff-count-text);
		pointer-events: none;

		&.staged {
			color: var(--clr-diff-selected-count-text);
		}
	}

	.table__title {
		user-select: none;
	}

	.draggable {
		cursor: grab;
	}

	.table__drag-handle {
		box-sizing: border-box;
		display: flex;
		position: absolute;
		top: 6px;
		right: 6px;
		align-items: center;
		justify-content: center;
		transform-origin: top right;
		color: var(--clr-text-2);
		opacity: 0;
		pointer-events: none;
		transition: opacity 0.15s;
	}

	.table__title-content {
		box-sizing: border-box;
		display: flex;
		align-items: center;
		padding: 4px 6px;
		border-bottom: 1px solid var(--clr-border-2);
		color: var(--clr-diff-count-text);
		font-size: 12px;
		text-wrap: nowrap;
	}

	/* CONTRAST MODIFIERS */
	/* Color blind-friendly overrides for medium (default) contrast */
	.colorblind-friendly {
		/* deletion (orange) */
		--clr-diff-deletion-line-bg: #fae8cc;
		--clr-diff-deletion-line-highlight: #f5d199;
		--clr-diff-deletion-count-bg: #f5deb3;
		--clr-diff-deletion-count-text: #bf590d;
		--clr-diff-deletion-count-border: #f2bf7f;
		/* addition (blue) */
		--clr-diff-addition-line-bg: #cce8fa;
		--clr-diff-addition-line-highlight: #99d1f5;
		--clr-diff-addition-count-bg: #b3def5;
		--clr-diff-addition-count-text: #0d59bf;
		--clr-diff-addition-count-border: #79b8eb;
	}

	/* Dark theme color-blind friendly overrides for medium (default) contrast */
	:global(.dark) .colorblind-friendly {
		/* deletion (orange) - darker variants for dark theme */
		--clr-diff-deletion-line-bg: #4d2e14;
		--clr-diff-deletion-line-highlight: #73401a;
		--clr-diff-deletion-count-bg: #66401f;
		--clr-diff-deletion-count-text: #ffbf66;
		--clr-diff-deletion-count-border: #a67333;
		/* addition (blue) - darker variants for dark theme */
		--clr-diff-addition-line-bg: #142e4d;
		--clr-diff-addition-line-highlight: #1a4d73;
		--clr-diff-addition-count-bg: #1f4066;
		--clr-diff-addition-count-text: #99d9ff;
		--clr-diff-addition-count-border: #2f6dab;
	}

	/* STRONG CONTRAST */
	.contrast-strong {
		/* deletion */
		--clr-diff-deletion-line-bg: #ffd4d8;
		--clr-diff-deletion-line-highlight: #ff9eaa;
		--clr-diff-deletion-count-bg: #ffb8c2;
		--clr-diff-deletion-count-text: #9b2030;
		--clr-diff-deletion-count-border: #e88a96;
		/* addition */
		--clr-diff-addition-line-bg: #a5f5d4;
		--clr-diff-addition-line-highlight: #5de0aa;
		--clr-diff-addition-count-bg: #7aeebe;
		--clr-diff-addition-count-text: #1a6b47;
		--clr-diff-addition-count-border: #4dd49a;
		/* locked */
		--clr-diff-locked-count-bg: #fae6ad;
		--clr-diff-locked-count-text: #bd7d12;
		--clr-diff-locked-count-border: #e6bf78;
	}

	/* Dark theme overrides for contrast-strong */
	:global(.dark) .contrast-strong {
		/* deletion */
		--clr-diff-deletion-line-bg: #6e1a28;
		--clr-diff-deletion-line-highlight: #b82e48;
		--clr-diff-deletion-count-bg: #8f2238;
		--clr-diff-deletion-count-text: #ffa0b4;
		--clr-diff-deletion-count-border: #c45068;
		/* addition */
		--clr-diff-addition-line-bg: #125747;
		--clr-diff-addition-line-highlight: #1c9d78;
		--clr-diff-addition-count-bg: #187a5f;
		--clr-diff-addition-count-text: #7aeebe;
		--clr-diff-addition-count-border: #3db888;
		/* locked */
		--clr-diff-locked-count-bg: #785217;
		--clr-diff-locked-count-text: #ebba66;
		--clr-diff-locked-count-border: #9b7338;
	}

	/* Color blind-friendly overrides for strong contrast */
	.contrast-strong.colorblind-friendly {
		/* deletion (orange) */
		--clr-diff-deletion-line-bg: #f0d4a0;
		--clr-diff-deletion-line-highlight: #e6b060;
		--clr-diff-deletion-count-bg: #eac080;
		--clr-diff-deletion-count-text: #a04000;
		--clr-diff-deletion-count-border: #d9a050;
		/* addition (blue) */
		--clr-diff-addition-line-bg: #a0d4f0;
		--clr-diff-addition-line-highlight: #60b0e6;
		--clr-diff-addition-count-bg: #84c4ee;
		--clr-diff-addition-count-text: #0040a0;
		--clr-diff-addition-count-border: #4e97ca;
	}

	/* Dark theme color-blind friendly overrides for strong contrast */
	:global(.dark) .contrast-strong.colorblind-friendly {
		/* deletion (orange) - darker variants for dark theme */
		--clr-diff-deletion-line-bg: #664020;
		--clr-diff-deletion-line-highlight: #996030;
		--clr-diff-deletion-count-bg: #805530;
		--clr-diff-deletion-count-text: #ffc060;
		--clr-diff-deletion-count-border: #c08840;
		/* addition (blue) - darker variants for dark theme */
		--clr-diff-addition-line-bg: #204066;
		--clr-diff-addition-line-highlight: #306099;
		--clr-diff-addition-count-bg: #305580;
		--clr-diff-addition-count-text: #80d0ff;
		--clr-diff-addition-count-border: #4088c0;
	}
</style>
