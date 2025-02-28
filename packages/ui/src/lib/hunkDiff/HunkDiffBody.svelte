<script lang="ts" module>
</script>

<script lang="ts">
	import HunkDiffRow from './HunkDiffRow.svelte';
	import LineSelection from './lineSelection.svelte';
	import { clickOutside } from '$lib/utils/clickOutside';
	import {
		type ContentSection,
		generateRows,
		type LineSelector,
		parserFromFilename
	} from '$lib/utils/diffParsing';
	import type { LineSelectionParams } from './lineSelection.svelte';

	interface Props {
		filePath: string;
		content: ContentSection[];
		tabSize?: number;
		wrapText?: boolean;
		inlineUnifiedDiffs?: boolean;
		selectedLines?: LineSelector[];
		onLineClick?: (params: LineSelectionParams) => void;
		clearLineSelection?: () => void;
		onQuoteSelection?: () => void;
		onCopySelection?: () => void;
		numberHeaderWidth?: number;
	}

	const {
		filePath,
		content,
		onLineClick,
		clearLineSelection,
		wrapText = true,
		tabSize = 4,
		inlineUnifiedDiffs = false,
		selectedLines,
		numberHeaderWidth,
		onCopySelection,
		onQuoteSelection
	}: Props = $props();

	const lineSelection = $derived(new LineSelection(onLineClick));
	const parser = $derived(parserFromFilename(filePath));
	const renderRows = $derived(
		generateRows(filePath, content, inlineUnifiedDiffs, parser, selectedLines)
	);

	$effect(() => lineSelection.setRows(renderRows));

	const hasSelectedLines = $derived(renderRows.filter((row) => row.isSelected).length > 0);

	let hoveringOverTable = $state(false);
	function handleClearSelection() {
		if (hasSelectedLines) clearLineSelection?.();
		lineSelection.onEnd();
	}
</script>

<tbody
	onmouseenter={() => (hoveringOverTable = true)}
	onmouseleave={() => (hoveringOverTable = false)}
	ontouchstart={(ev) => lineSelection.onTouchStart(ev)}
	ontouchmove={(ev) => lineSelection.onTouchMove(ev)}
	ontouchend={() => lineSelection.onEnd()}
	use:clickOutside={{
		handler: handleClearSelection
	}}
>
	{#each renderRows as row, idx}
		<HunkDiffRow
			{idx}
			{row}
			{onLineClick}
			{lineSelection}
			{tabSize}
			{wrapText}
			{numberHeaderWidth}
			{onQuoteSelection}
			{onCopySelection}
			clearLineSelection={handleClearSelection}
			{hoveringOverTable}
		/>
	{/each}
</tbody>

<style lang="postcss">
	tbody {
		z-index: var(--z-lifted);
	}
</style>
