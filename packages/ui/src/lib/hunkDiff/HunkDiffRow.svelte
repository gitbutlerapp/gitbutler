<script lang="ts" module>
	import { isTouchDevice } from '$lib/utils/browserAgent';

	export function getHunkLineId(rowEncodedId: DiffFileLineId): string {
		return `hunk-line-${rowEncodedId}`;
	}
</script>

<script lang="ts">
	import Button from '$lib/Button.svelte';
	import {
		CountColumnSide,
		SectionType,
		type DiffFileLineId,
		type Row
	} from '$lib/utils/diffParsing';
	import type LineSelection from './lineSelection.svelte';
	import type { LineSelectionParams } from './lineSelection.svelte';

	interface Props {
		idx: number;
		row: Row;
		onLineClick?: (params: LineSelectionParams) => void;
		clearLineSelection?: () => void;
		lineSelection: LineSelection;
		tabSize: number;
		wrapText: boolean;
		diffFont?: string;
		numberHeaderWidth?: number;
		onQuoteSelection?: () => void;
		onCopySelection?: () => void;
		hoveringOverTable: boolean;
	}

	const {
		idx,
		row,
		onLineClick,
		lineSelection,
		tabSize,
		wrapText,
		diffFont = 'var(--fontfamily-mono)',
		clearLineSelection,
		numberHeaderWidth,
		onQuoteSelection,
		onCopySelection,
		hoveringOverTable
	}: Props = $props();

	const touchDevice = isTouchDevice();

	let rowElement = $state<HTMLTableRowElement>();
	let overflowMenuHeight = $state<number>(0);

	const rowTop = $derived(rowElement?.getBoundingClientRect().top);
	const rowLeft = $derived(rowElement?.getBoundingClientRect().left);
	const rowWidth = $derived(rowElement?.getBoundingClientRect().width);
	const rowHeight = $derived(rowElement?.getBoundingClientRect().height);

	$effect(() => {
		if (
			lineSelection.touchStart !== undefined &&
			rowTop !== undefined &&
			rowLeft !== undefined &&
			numberHeaderWidth !== undefined &&
			rowHeight !== undefined
		) {
			const rowTouchStartY =
				lineSelection.touchStart.y > rowTop && lineSelection.touchStart.y < rowTop + rowHeight;
			const rowTouchStartX =
				lineSelection.touchStart.x > rowLeft &&
				lineSelection.touchStart.x < rowLeft + numberHeaderWidth;
			if (rowTouchStartY && rowTouchStartX) {
				lineSelection.touchSelectionStart(row, idx);
			}

			if (lineSelection.touchMove !== undefined) {
				const rowTouchEndsY =
					lineSelection.touchMove.y > rowTop && lineSelection.touchMove.y < rowTop + rowHeight;
				const rowTouchEndsX =
					lineSelection.touchMove.x > rowLeft &&
					lineSelection.touchMove.x < rowLeft + numberHeaderWidth;
				if (rowTouchEndsY && rowTouchEndsX) {
					lineSelection.touchSelectionEnd(row, idx);
				}
			}
		}
	});
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

<tr
	bind:this={rowElement}
	id={getHunkLineId(row.encodedLineId)}
	class="table__row"
	class:selected={row.isSelected}
	data-no-drag
	style="--diff-font: {diffFont};"
>
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
		onclick={() => {
			if (!row.isSelected) clearLineSelection?.();
		}}
	>
		<div class="table__row-header">
			{#if row.isSelected}
				<div
					class="table__selected-row-overlay"
					class:is-first={row.isFirstOfSelectionGroup}
					class:is-last={row.isLastOfSelectionGroup}
					style="--number-col-width: {numberHeaderWidth}px; --width: {rowWidth}px; --height: {rowHeight}px;"
				></div>
			{/if}

			{#if row.isLastSelected}
				<div
					bind:clientHeight={overflowMenuHeight}
					class="table__selected-row-overflow-menu"
					class:visible={hoveringOverTable || touchDevice}
					style="--number-col-width: {numberHeaderWidth}px; --height: {rowHeight}px; --overflow-menu-height: {overflowMenuHeight}px;"
				>
					{#if onQuoteSelection}
						<div class="button-wrapper">
							<Button
								icon="text-quote"
								style="neutral"
								kind="ghost"
								size="button"
								tooltip="Quote"
								onclick={onQuoteSelection}
							/>
						</div>
					{/if}

					<div class="button-wrapper">
						<Button
							icon="copy-small"
							style="neutral"
							kind="ghost"
							size="button"
							tooltip="Copy"
							onclick={onCopySelection}
						/>
					</div>
				</div>
			{/if}

			{@html row.tokens.join('')}
		</div>
	</td>
</tr>

<style lang="postcss">
	td,
	tr {
		padding: 0;
		margin: 0;
		user-select: none;
		font-family: var(--diff-font);
	}

	.table__textContent {
		width: 100%;
		font-size: 12px;
		padding-left: 4px;
		line-height: 1.25;
		tab-size: var(--tab-size);
		white-space: pre;
		user-select: text;
		cursor: text;
		text-wrap: var(--wrap);
		border-left: 1px solid var(--clr-border-2);
	}

	.table__row-header {
		position: relative;
	}

	.table__selected-row-overlay {
		z-index: var(--z-lifted);
		position: absolute;
		pointer-events: none;
		top: 0;

		/* border + left padding + number column width */
		--offset: calc(2px + 4px + var(--number-col-width));

		left: calc(var(--offset) * -1);
		width: calc(var(--width) + 1px);
		height: var(--height);
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

	.table__selected-row-overflow-menu {
		z-index: var(--z-lifted);
		position: absolute;
		top: calc(var(--height) - var(--overflow-menu-height) - 6px);
		left: 0;
		display: flex;
		pointer-events: none;
		background: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		box-shadow: var(--fx-shadow-s);
		transition: opacity var(--transition-medium);

		.button-wrapper:not(:last-child) {
			border-right: 1px solid var(--clr-border-2);
		}

		&.visible {
			opacity: 1;
			pointer-events: all;
		}
	}

	.button-wrapper {
		display: flex;
	}

	.table__numberColumn {
		color: var(--clr-diff-count-text);
		border-color: var(--clr-diff-count-border);
		background-color: var(--clr-diff-count-bg);
		font-size: 11px;
		text-align: center;
		padding: 0 4px;
		text-align: right;
		vertical-align: top;
		user-select: none;
		touch-action: none;

		position: sticky;
		left: calc(var(--number-col-width));
		width: var(--number-col-width);
		min-width: var(--number-col-width);

		border-right: 1px solid var(--clr-border-2);

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

		&.is-before.is-last {
			border-bottom-left-radius: var(--radius-s);
		}

		&.clickable {
			cursor: pointer;
		}
	}

	.table__numberColumn:first-of-type {
		width: var(--number-col-width);
		min-width: var(--number-col-width);
		left: 0px;
	}

	/* DIFF LINE */
	.diff-line-addition {
		background-color: var(--clr-diff-addition-line-bg);
	}

	.diff-line-deletion {
		background-color: var(--clr-diff-deletion-line-bg);
	}
</style>
