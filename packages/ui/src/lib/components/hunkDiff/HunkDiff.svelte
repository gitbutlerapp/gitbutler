<script lang="ts" module>
	import { type LineSelectionParams } from '$components/hunkDiff/lineSelection.svelte';
	export type LineClickParams = LineSelectionParams;
</script>

<script lang="ts">
	import Icon from '$components/Icon.svelte';
	import HunkDiffBody from '$components/hunkDiff/HunkDiffBody.svelte';
	import ScrollableContainer from '$components/scroll/ScrollableContainer.svelte';
	import {
		type ContentSection,
		type DependencyLock,
		type LineId,
		type LineLock,
		type LineSelector,
		parseHunk
	} from '$lib/utils/diffParsing';
	import { isDefined } from '$lib/utils/typeguards';
	import type { ContextMenuParams } from '$components/hunkDiff/HunkDiffRow.svelte';
	import type { Snippet } from 'svelte';
	interface Props {
		filePath: string;
		hunkStr: string;
		tabSize?: number;
		wrapText?: boolean;
		diffFont?: string;
		diffLigatures?: boolean;
		inlineUnifiedDiffs?: boolean;
		diffContrast?: 'light' | 'medium' | 'strong';
		staged?: boolean;
		stagedLines?: LineId[];
		hideCheckboxes?: boolean;
		selectedLines?: LineSelector[];
		lineLocks?: LineLock[];
		draggingDisabled?: boolean;
		onChangeStage?: (staged: boolean) => void;
		onLineClick?: (params: LineSelectionParams) => void;
		clearLineSelection?: (fileName: string) => void;
		onQuoteSelection?: () => void;
		onCopySelection?: (contentSections: ContentSection[]) => void;
		handleLineContextMenu?: (params: ContextMenuParams) => void;
		clickOutsideExcludeElement?: HTMLElement;
		lockWarning?: Snippet<[DependencyLock[]]>;
	}

	const {
		filePath,
		hunkStr,
		tabSize = 4,
		wrapText = true,
		diffFont = 'var(--fontfamily-mono)',
		diffLigatures = true,
		diffContrast = 'medium',
		inlineUnifiedDiffs = false,
		staged,
		stagedLines,
		hideCheckboxes,
		selectedLines,
		lineLocks,
		onChangeStage,
		onLineClick,
		clearLineSelection,
		onCopySelection,
		onQuoteSelection,
		handleLineContextMenu,
		clickOutsideExcludeElement,
		draggingDisabled,
		lockWarning
	}: Props = $props();

	const BORDER_WIDTH = 1;

	let numberHeaderWidth = $state<number>(0);

	const hunk = $derived(parseHunk(hunkStr));

	function handleCopySelection() {
		onCopySelection?.(hunk.contentSections);
	}

	const hunkSummary = $derived(
		`@@ -${hunk.oldStart},${hunk.oldLines} +${hunk.newStart},${hunk.newLines} @@`
	);
	const showingCheckboxes = $derived(!hideCheckboxes && staged !== undefined);
	const hunkHasLocks = $derived(lineLocks && lineLocks.length > 0);
	const colspan = $derived(showingCheckboxes || hunkHasLocks ? 3 : 2);
	let tableWrapperElem = $state<HTMLElement>();

	function handleHunkContextMenu(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();

		if (handleLineContextMenu) {
			handleLineContextMenu({
				event: e,
				beforeLineNumber: undefined,
				afterLineNumber: undefined
			});
		}
	}
</script>

<div
	bind:this={tableWrapperElem}
	class="table__wrapper contrast-{diffContrast}"
	style="--tab-size: {tabSize}; --diff-font: {diffFont};"
	style:font-variant-ligatures={diffLigatures ? 'common-ligatures' : 'none'}
>
	{#if !draggingDisabled}
		<div class="table__drag-handle">
			<Icon name="draggable" />
		</div>
	{/if}
	<ScrollableContainer horz whenToShow="always" padding={{ left: numberHeaderWidth }}>
		<!-- <div style="overflow: auto; max-height: 600px;"> -->
		<table class="table__section">
			<thead class="table__title" class:draggable={!draggingDisabled}>
				<tr>
					<th
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
				<!-- We need to await the table wrapper to be mounted in order to set the array of elements
			 to ignore when clicking outside.
			 This is the case because the clickOutside handler needs to know which elements to ignore
			 at mount time. Reactive updates to the array will not work as expected. -->
				{@const elemetsToIgnoreInClickOutside = [
					clickOutsideExcludeElement,
					tableWrapperElem
				].filter(isDefined)}
				<HunkDiffBody
					comment={hunk.comment}
					{filePath}
					content={hunk.contentSections}
					{onLineClick}
					clearLineSelection={() => clearLineSelection?.(filePath)}
					{wrapText}
					{tabSize}
					{diffFont}
					{inlineUnifiedDiffs}
					{selectedLines}
					{lineLocks}
					{numberHeaderWidth}
					onCopySelection={onCopySelection && handleCopySelection}
					{onQuoteSelection}
					{staged}
					{stagedLines}
					{hideCheckboxes}
					{handleLineContextMenu}
					clickOutsideExcludeElements={elemetsToIgnoreInClickOutside}
					{lockWarning}
				/>
			{/if}
		</table>
		<!-- </div> -->
	</ScrollableContainer>
</div>

<style lang="postcss">
	.table__wrapper {
		position: relative;
		width: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-diff-line-bg);

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
		/* position: sticky; */
		top: 0;
		left: 0;
		height: 28px;
	}

	.table__checkbox-container {
		box-sizing: border-box;
		border-right: 1px solid var(--clr-border-2);
		border-bottom: 1px solid var(--clr-border-2);
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
		color: var(--clr-diff-count-checkmark);
		pointer-events: none;

		&.staged {
			color: var(--clr-diff-selected-count-checkmark);
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

	.table__lock {
		box-sizing: border-box;
		display: flex;
		position: fixed;
		top: 6px;
		right: 6px;
		align-items: center;
		justify-content: center;
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-warn-soft);
		color: var(--clr-text-2);
		pointer-events: none;
		transition: transform var(--transition-medium);
	}

	.table__title-content {
		box-sizing: border-box;
		display: flex;
		/* position: absolute;
		top: var(--top);
		left: var(--number-col-width); */
		align-items: center;
		/* width: calc(var(--table-width) - var(--number-col-width));
		height: calc(100% + var(--border-width) * 2); */
		padding: 4px 6px;
		border-bottom: 1px solid var(--clr-border-2);
		/* border-top-right-radius: var(--radius-m); */
		color: var(--clr-text-2);
		font-size: 12px;
		text-wrap: nowrap;
	}

	/* HIDDINE LINES STATE */

	.table__hidden-rows {
		display: none;
	}

	.table__hiddenRows__count {
		width: 24px;
		background-color: var(--clr-diff-count-bg);

		&:nth-child(2) {
			border-right: 1px solid var(--clr-border-2);
		}
	}

	.table__hiddenRows__content {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 40px 24px;
		gap: 14px;
		background-color: var(--clr-bg-1-muted);
	}

	.table__hiddenRows__caption {
		color: var(--clr-text-2);
		text-align: center;
	}

	/* CONTRAST MODIFIERS */
	.contrast-light {
		--clr-diff-count-text: var('--', var(--clr-diff-count-text));
		/* deletion */
		--clr-diff-deletion-line-bg: var('--', var(--clr-diff-deletion-line-bg));
		--clr-diff-deletion-line-highlight: var('--', var(--clr-diff-deletion-line-highlight));
		--clr-diff-deletion-count-bg: var('--', var(--clr-diff-deletion-count-bg));
		--clr-diff-deletion-count-text: var('--', var(--clr-diff-deletion-count-text));
		--clr-diff-deletion-count-border: var('--', var(--clr-diff-deletion-count-border));
		/* addition */
		--ctx-diff-addition-line-bg: var('--', var(--clr-diff-addition-line-bg));
		--clr-diff-addition-line-highlight: var('--', var(--clr-diff-addition-line-highlight));
		--clr-diff-addition-count-bg: var('--', var(--clr-diff-addition-count-bg));
		--clr-diff-addition-count-text: var('--', var(--clr-diff-addition-count-text));
		--clr-diff-addition-count-border: var('--', var(--clr-diff-addition-count-border));
		/* locked */
		--clr-diff-locked-count-bg: var('--', var(--clr-diff-locked-count-bg));
		--clr-diff-locked-count-text: var('--', var(--clr-diff-locked-count-text));
		--clr-diff-locked-count-border: var('--', var(--clr-diff-locked-count-border));
	}

	.contrast-medium {
		--clr-diff-count-text: var(--clr-diff-count-text-contrast-2);
		/* deletion */
		--clr-diff-deletion-line-bg: var(--clr-diff-deletion-contrast-2-line-bg);
		--clr-diff-deletion-line-highlight: var(--clr-diff-deletion-contrast-2-line-highlight);
		--clr-diff-deletion-count-bg: var(--clr-diff-deletion-contrast-2-count-bg);
		--clr-diff-deletion-count-text: var(--clr-diff-deletion-contrast-2-count-text);
		--clr-diff-deletion-count-border: var(--clr-diff-deletion-contrast-2-count-border);
		/* addition */
		--clr-diff-addition-line-bg: var(--clr-diff-addition-contrast-2-line-bg);
		--clr-diff-addition-line-highlight: var(--clr-diff-addition-contrast-2-line-highlight);
		--clr-diff-addition-count-bg: var(--clr-diff-addition-contrast-2-count-bg);
		--clr-diff-addition-count-text: var(--clr-diff-addition-contrast-2-count-text);
		--clr-diff-addition-count-border: var(--clr-diff-addition-contrast-2-count-border);
		/* locked */
		--clr-diff-locked-count-bg: var(--clr-diff-locked-contrast-2-count-bg);
		--clr-diff-locked-count-text: var(--clr-diff-locked-contrast-2-count-text);
		--clr-diff-locked-count-border: var(--clr-diff-locked-contrast-2-count-border);
	}

	.contrast-strong {
		--clr-diff-count-text: var(--clr-diff-count-text-contrast-3);
		/* deletion */
		--clr-diff-deletion-line-bg: var(--clr-diff-deletion-contrast-3-line-bg);
		--clr-diff-deletion-line-highlight: var(--clr-diff-deletion-contrast-3-line-highlight);
		--clr-diff-deletion-count-bg: var(--clr-diff-deletion-contrast-3-count-bg);
		--clr-diff-deletion-count-text: var(--clr-diff-deletion-contrast-3-count-text);
		--clr-diff-deletion-count-border: var(--clr-diff-deletion-contrast-3-count-border);
		/* addition */
		--clr-diff-addition-line-bg: var(--clr-diff-addition-contrast-3-line-bg);
		--clr-diff-addition-line-highlight: var(--clr-diff-addition-contrast-3-line-highlight);
		--clr-diff-addition-count-bg: var(--clr-diff-addition-contrast-3-count-bg);
		--clr-diff-addition-count-text: var(--clr-diff-addition-contrast-3-count-text);
		--clr-diff-addition-count-border: var(--clr-diff-addition-contrast-3-count-border);
		/* locked */
		--clr-diff-locked-count-bg: var(--clr-diff-locked-contrast-3-count-bg);
		--clr-diff-locked-count-text: var(--clr-diff-locked-contrast-3-count-text);
		--clr-diff-locked-count-border: var(--clr-diff-locked-contrast-3-count-border);
	}
</style>
