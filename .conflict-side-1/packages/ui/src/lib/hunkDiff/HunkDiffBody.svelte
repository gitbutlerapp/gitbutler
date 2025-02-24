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
		diffFont?: string;
		tabSize?: number;
		wrapText?: boolean;
		inlineUnifiedDiffs?: boolean;
		selectedLines?: LineSelector[];
		diffContrast?: 'light' | 'medium' | 'strong';
		onLineClick?: (params: LineSelectionParams) => void;
		clearLineSelection?: () => void;
		onQuoteSelection?: () => void;
		onCopySelection?: () => void;
		numberHeaderWidth?: number;
	}

	const {
		diffFont,
		filePath,
		content,
		onLineClick,
		clearLineSelection,
		wrapText = true,
		tabSize = 4,
		inlineUnifiedDiffs = false,
		selectedLines,
		diffContrast = 'medium',
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
</script>

<tbody
	class="contrast-{diffContrast}"
	style="--diff-font: {diffFont};"
	onmouseenter={() => (hoveringOverTable = true)}
	onmouseleave={() => (hoveringOverTable = false)}
	use:clickOutside={{
		handler: () => {
			if (hasSelectedLines) clearLineSelection?.();
		}
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
			{hasSelectedLines}
			{numberHeaderWidth}
			{onQuoteSelection}
			{onCopySelection}
			{hoveringOverTable}
		/>
	{/each}
</tbody>

<style lang="postcss">
	tbody {
		z-index: var(--z-lifted);
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
	}
</style>
