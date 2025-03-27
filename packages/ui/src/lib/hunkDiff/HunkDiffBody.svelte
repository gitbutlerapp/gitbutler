<script lang="ts" module>
</script>

<script lang="ts">
	import HunkDiffRow, { type ContextMenuParams } from '$lib/hunkDiff/HunkDiffRow.svelte';
	import LineSelection from '$lib/hunkDiff/lineSelection.svelte';
	import { clickOutside } from '$lib/utils/clickOutside';
	import {
		type ContentSection,
		generateRows,
		type LineId,
		type LineSelector,
		parserFromFilename,
		type Row
	} from '$lib/utils/diffParsing';
	import type { LineSelectionParams } from '$lib/hunkDiff/lineSelection.svelte';

	interface Props {
		filePath: string;
		content: ContentSection[];
		tabSize?: number;
		wrapText?: boolean;
		diffFont?: string;
		inlineUnifiedDiffs?: boolean;
		selectedLines?: LineSelector[];
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
	}

	const {
		filePath,
		content,
		onLineClick,
		clearLineSelection,
		wrapText = true,
		diffFont,
		tabSize = 4,
		inlineUnifiedDiffs = false,
		selectedLines,
		numberHeaderWidth,
		onCopySelection,
		onQuoteSelection,
		staged,
		stagedLines,
		hideCheckboxes,
		handleLineContextMenu,
		clickOutsideExcludeElement
	}: Props = $props();

	const lineSelection = new LineSelection();
	const parser = $derived(parserFromFilename(filePath));
	const renderRows = $derived(
		generateRows(filePath, content, inlineUnifiedDiffs, parser, selectedLines)
	);
	const clickable = $derived(!!onLineClick);

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
	{#each renderRows as row, idx}
		<HunkDiffRow
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
		/>
	{/each}
</tbody>

<style lang="postcss">
	tbody {
		z-index: var(--z-lifted);
	}
</style>
