<script lang="ts" module>
	export function getHunkLineId(rowEncodedId: DiffFileLineId): string {
		return `hunk-line-${rowEncodedId}`;
	}
</script>

<script lang="ts">
	import LineSelection from './lineSelection.svelte';
	import Button from '$lib/Button.svelte';
	import {
		type ContentSection,
		CountColumnSide,
		type DiffFileLineId,
		generateRows,
		type LineSelector,
		parserFromFilename,
		type Row,
		SectionType
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
		onQuoteSelection?: () => void;
		onCopySelection?: () => void;
		numberHeaderWidth?: number;
	}

	const {
		diffFont,
		filePath,
		content,
		onLineClick,
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
</script>

{#snippet countColumn(row: Row, side: CountColumnSide, idx: number)}
	<td
		class="table__numberColumn"
		data-no-drag
		class:diff-line-deletion={row.type === SectionType.RemovedLines}
		class:diff-line-addition={row.type === SectionType.AddedLines}
		class:clickable={onLineClick}
		align="center"
		class:is-last={row.isLast}
		class:is-before={side === CountColumnSide.Before}
		onmousedown={(ev) => lineSelection.onStart(ev, row, idx)}
		onmouseenter={(ev) => lineSelection.onMoveOver(ev, row, idx)}
		onmouseup={() => lineSelection.onEnd()}
	>
		{side === CountColumnSide.Before ? row.beforeLineNumber : row.afterLineNumber}
	</td>
{/snippet}

<tbody class="contrast-{diffContrast}" style="--diff-font: {diffFont};">
	{#each renderRows as row, idx}
		<tr id={getHunkLineId(row.encodedLineId)} class="table__row" data-no-drag>
			{@render countColumn(row, CountColumnSide.Before, idx)}
			{@render countColumn(row, CountColumnSide.After, idx)}
			<td
				class="table__textContent"
				style="--tab-size: {tabSize}; --wrap: {wrapText ? 'wrap' : 'nowrap'}"
				class:readonly={true}
				data-no-drag
				class:diff-line-deletion={row.type === SectionType.RemovedLines}
				class:diff-line-addition={row.type === SectionType.AddedLines}
				class:selected={row.isSelected}
				class:is-last={row.isLast}
			>
				{#if row.isSelected}
					<div
						class="table__selected-row-overlay"
						class:is-first={row.isFirstOfSelectionGroup}
						class:is-last={row.isLastOfSelectionGroup}
					></div>
				{/if}

				{#if row.isLastSelected}
					<div
						class="table__selected-row-overflow-menu"
						style="--number-col-width: {numberHeaderWidth}px;"
					>
						<div class="button-wrapper">
							<Button
								icon="text-quote"
								style="neutral"
								kind="ghost"
								size="button"
								onclick={onQuoteSelection}
							/>
						</div>
						<div class="button-wrapper">
							<Button
								icon="copy-small"
								style="neutral"
								kind="ghost"
								size="button"
								onclick={onCopySelection}
							/>
						</div>
					</div>
				{/if}

				{@html row.tokens.join('')}
			</td>
		</tr>
	{/each}
</tbody>

<style lang="postcss">
	tbody {
		z-index: var(--z-lifted);
	}

	td,
	tr {
		padding: 0;
		margin: 0;
	}

	.table__row {
		position: relative;
	}

	.table__textContent {
		z-index: var(--z-lifted);
		width: 100%;

		color: var(--clr-text-1, #1a1614);
		font-family: 'Geist Mono';
		font-size: 12px;
		font-style: normal;
		font-weight: 400;
		line-height: 120%; /* 14.4px */

		padding-left: 4px;
		tab-size: var(--tab-size);
		white-space: pre;
		user-select: text;
		cursor: text;
		text-wrap: var(--wrap);
		border-left: 1px solid var(--clr-border-2);
	}

	.table__selected-row-overlay {
		z-index: var(--z-floating);
		position: absolute;
		pointer-events: none;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		box-sizing: border-box;

		border-left: 1px solid var(--clr-theme-warn-element);
		border-right: 1px solid var(--clr-theme-warn-element);

		background: color-mix(in srgb, var(--clr-btn-warn-outline-bg), transparent 30%);
		mix-blend-mode: multiply;

		&.is-first {
			border-top: 1px solid var(--clr-theme-warn-element);
		}

		&.is-last {
			border-bottom: 1px solid var(--clr-theme-warn-element);
		}
	}

	tbody:hover .table__selected-row-overflow-menu {
		opacity: 1;
		pointer-events: all;
	}

	.table__selected-row-overflow-menu {
		z-index: var(--z-modal);
		position: absolute;
		bottom: 4px;
		left: calc(var(--number-col-width) + 4px);

		display: flex;
		pointer-events: none;
		gap: 0;
		background: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);

		/* shadow/s */
		box-shadow: 0px 4px 14px 0px rgba(0, 0, 0, 0.06);

		opacity: 0;
		transition: opacity var(--transition-medium);

		.button-wrapper:not(:last-child) {
			border-right: 1px solid var(--clr-border-2);
		}
	}

	.table__numberColumn {
		color: var(--clr-diff-count-text, #b4afac);
		font-family: 'Geist Mono';
		font-size: 11px;
		font-style: normal;
		font-weight: 400;
		line-height: 120%; /* 13.2px */

		border-color: var(--clr-diff-count-border);
		background-color: var(--clr-diff-count-bg);
		padding: 0 4px;
		text-align: right;
		vertical-align: top;
		user-select: none;

		box-sizing: border-box;
		position: sticky;

		&.diff-line-addition {
			background-color: var(--clr-diff-addition-count-bg);
			color: var(--clr-diff-addition-count-text);
			border-color: var(--clr-diff-addition-count-border);
		}

		&.diff-line-deletion {
			background-color: var(--clr-diff-deletion-count-bg);
			color: var(--clr-diff-deletion-count-text);
			border-color: var(--clr-diff-deletion-count-border);
		}

		&.selected {
			background-color: var(--clr-diff-selected-count-bg);
			color: var(--clr-diff-selected-count-text);
			border-color: var(--clr-diff-selected-count-border);
		}

		&.is-before.is-last {
			border-bottom-left-radius: var(--radius-s);
		}

		&.clickable {
			cursor: pointer;
		}
	}

	.table__numberColumn:first-of-type {
		left: 0px;
	}

	/* DIFF LINE */
	.diff-line-addition {
		background-color: var(--clr-diff-addition-line-bg);
	}

	.diff-line-deletion {
		background-color: var(--clr-diff-deletion-line-bg);
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
