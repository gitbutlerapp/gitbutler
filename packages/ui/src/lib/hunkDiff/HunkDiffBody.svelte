<script lang="ts">
	import HunkDiffRow, { type ContextMenuParams } from '$lib/hunkDiff/HunkDiffRow.svelte';
	import LineSelection from '$lib/hunkDiff/lineSelection.svelte';
	import { clickOutside } from '$lib/utils/clickOutside';
	import {
		type ContentSection,
		type DependencyLock,
		generateRows,
		type LineId,
		lineIdKey,
		type LineLock,
		type LineSelector,
		parserFromFilename,
		type Row,
		SectionType
	} from '$lib/utils/diffParsing';
	import { intersectionObserver } from '$lib/utils/intersectionObserver';
	import type { LineSelectionParams } from '$lib/hunkDiff/lineSelection.svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		filePath: string;
		content: ContentSection[];
		tabSize?: number;
		wrapText?: boolean;
		diffFont?: string;
		inlineUnifiedDiffs?: boolean;
		selectedLines?: LineSelector[];
		lineLocks?: LineLock[];
		onLineClick?: (params: LineSelectionParams) => void;
		clearLineSelection?: () => void;
		onQuoteSelection?: () => void;
		onCopySelection?: () => void;
		numberHeaderWidth?: number;
		staged?: boolean;
		stagedLines?: LineId[];
		hideCheckboxes?: boolean;
		handleLineContextMenu?: (params: ContextMenuParams) => void;
		clickOutsideExcludeElement?: HTMLElement;
		comment?: string;
		lockWarning?: Snippet<[DependencyLock[]]>;
	}

	const {
		comment,
		filePath,
		content,
		onLineClick,
		clearLineSelection,
		wrapText = true,
		diffFont,
		tabSize = 4,
		inlineUnifiedDiffs = false,
		selectedLines,
		lineLocks,
		numberHeaderWidth,
		onCopySelection,
		onQuoteSelection,
		staged,
		stagedLines,
		hideCheckboxes,
		handleLineContextMenu,
		clickOutsideExcludeElement,
		lockWarning
	}: Props = $props();

	const lineSelection = new LineSelection();
	const parser = $derived(parserFromFilename(filePath));
	const renderRows = $derived(
		generateRows(filePath, content, inlineUnifiedDiffs, parser, selectedLines, lineLocks)
	);
	const clickable = $derived(!!onLineClick);
	const maxLineNumber = $derived.by(() => {
		if (renderRows.length === 0) return 0;

		const lastRow = renderRows.at(-1);
		if (!lastRow) return 0;

		if (lastRow.beforeLineNumber === undefined && lastRow.afterLineNumber === undefined) {
			return 0;
		}

		if (lastRow.beforeLineNumber === undefined) {
			return lastRow.afterLineNumber;
		}

		if (lastRow.afterLineNumber === undefined) {
			return lastRow.beforeLineNumber;
		}
		return Math.max(lastRow.beforeLineNumber, lastRow.afterLineNumber);
	});

	function getGutterMinWidth(max: number | undefined) {
		if (!max) {
			return 1;
		}
		if (max >= 10000) return 2.5;
		if (max >= 1000) return 2;
		if (max >= 100) return 1.5;
		if (max >= 10) return 1.25;
		return 1;
	}

	const minWidth = $derived(getGutterMinWidth(maxLineNumber));

	$effect(() => lineSelection.setRows(renderRows));
	$effect(() => lineSelection.setOnLineClick(onLineClick));

	const hasSelectedLines = $derived(renderRows.filter((row) => row.isSelected).length > 0);

	let hoveringOverTable = $state(false);
	function handleClearSelection() {
		if (hasSelectedLines) clearLineSelection?.();
		lineSelection.onEnd();
	}

	function getStageState(row: Row): boolean | undefined {
		if (staged === undefined) return undefined;
		if (stagedLines === undefined || stagedLines.length === 0) return staged;
		return stagedLines.some(
			(line) => line.newLine === row.afterLineNumber && line.oldLine === row.beforeLineNumber
		);
	}

	const showingExtraColumn = $derived(staged !== undefined && !hideCheckboxes);
	const commentNumericColSpan = $derived(showingExtraColumn ? 3 : 2);

	const commentRows = $derived.by(() => {
		if (!comment) return undefined;
		return generateRows(
			filePath,
			[
				{
					sectionType: SectionType.Context,
					lines: [{ beforeLineNumber: 0, afterLineNumber: 0, content: comment }]
				}
			],
			false,
			parser,
			undefined,
			undefined
		);
	});

	const commentRow = $derived(commentRows?.[0]);

	/* TODO: Move utility function from `apps/desktop` into this UI library. */
	function chunk<T>(arr: T[], size: number) {
		return Array.from({ length: Math.ceil(arr.length / size) }, (_v, i) =>
			arr.slice(i * size, i * size + size)
		);
	}

	/* Number of lines grouped together for intersection observer purposes. */
	const chunkLength = 5;
	/* The assumed height of a row, used to set height before rows have rendered. */
	const defaultChunkHeight = 18;

	const chunkedRows = $derived(chunk(renderRows, chunkLength));

	/* Bound array of booleans used to control rendering of rows. */
	let chunkVisibility = $state<boolean[]>([]);
	/* Bound array of chunk heights, for accurate scrolling. */
	let chunkHeight = $state<number[]>([]);

	$effect(() => {
		if (chunkedRows) {
			chunkVisibility.length = chunkedRows.length;
			chunkHeight.length = chunkedRows.length;
		}
	});
</script>

<tbody
	onmouseenter={() => (hoveringOverTable = true)}
	onmouseleave={() => (hoveringOverTable = false)}
	ontouchstart={(ev) => lineSelection.onTouchStart(ev)}
	ontouchmove={(ev) => lineSelection.onTouchMove(ev)}
	ontouchend={() => lineSelection.onEnd()}
	use:clickOutside={{
		handler: handleClearSelection,
		excludeElement: clickOutsideExcludeElement
	}}
>
	{#if commentRow}
		<tr>
			<td class="diff-comment__number-column" colspan={commentNumericColSpan}>comment</td>
			<td style="--tab-size: {tabSize};" class="diff-comment">
				{@html commentRow.tokens.join('')}
			</td>
		</tr>
	{/if}
</tbody>

{#each chunkedRows as renderRows, i}
	<tbody
		onmouseenter={() => (hoveringOverTable = true)}
		onmouseleave={() => (hoveringOverTable = false)}
		ontouchstart={(ev) => lineSelection.onTouchStart(ev)}
		ontouchmove={(ev) => lineSelection.onTouchMove(ev)}
		ontouchend={() => lineSelection.onEnd()}
		bind:clientHeight={chunkHeight[i]}
		use:clickOutside={{
			handler: handleClearSelection,
			excludeElement: clickOutsideExcludeElement
		}}
		use:intersectionObserver={{
			callback: (entries) => {
				chunkVisibility[i] = !!entries?.isIntersecting;
			},
			options: { threshold: 0 }
		}}
	>
		{#if chunkVisibility[i]}
			{#each renderRows as row, idx (lineIdKey( { oldLine: row.beforeLineNumber, newLine: row.afterLineNumber } ))}
				<HunkDiffRow
					{minWidth}
					{idx}
					{row}
					{clickable}
					{lineSelection}
					{tabSize}
					{wrapText}
					{diffFont}
					{numberHeaderWidth}
					{onQuoteSelection}
					{onCopySelection}
					clearLineSelection={handleClearSelection}
					{hoveringOverTable}
					staged={getStageState(row)}
					{hideCheckboxes}
					{handleLineContextMenu}
					{lockWarning}
				/>
			{/each}
		{:else}
			<tr>
				<td
					style:height={chunkHeight[i]
						? chunkHeight[i] + 'px'
						: chunkLength * defaultChunkHeight + 'px'}
				>
				</td>
			</tr>
		{/if}
	</tbody>
{/each}

<style lang="postcss">
	tbody {
		z-index: var(--z-lifted);
	}
	.diff-comment {
		width: 100%;
		font-size: 12px;
		padding-left: 4px;
		line-height: 1.25;
		tab-size: var(--tab-size);
		white-space: pre;
		user-select: text;
		cursor: text;
		text-wrap: wrap;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-diff-count-bg);
	}

	.diff-comment__number-column {
		color: var(--clr-diff-count-text);
		border-right: 1px solid var(--clr-border-2);
		border-bottom: 1px solid var(--clr-diff-count-border);
		background-color: var(--clr-diff-count-bg);
		font-size: 11px;
		padding: 4px 4px;
		text-align: right;
		vertical-align: middle;
		user-select: none;
		touch-action: none;
	}
</style>
