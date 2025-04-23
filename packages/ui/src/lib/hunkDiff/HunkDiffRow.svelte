<script lang="ts" module>
	import { isTouchDevice } from '$lib/utils/browserAgent';

	export function getHunkLineId(rowEncodedId: DiffFileLineId): string {
		return `hunk-line-${rowEncodedId}`;
	}

	export type ContextMenuParams = {
		event: MouseEvent;
		beforeLineNumber: number | undefined;
		afterLineNumber: number | undefined;
	};
</script>

<script lang="ts">
	import Button from '$lib/Button.svelte';
	import Checkbox from '$lib/Checkbox.svelte';
	import Icon from '$lib/Icon.svelte';
	import InfoButton from '$lib/InfoButton.svelte';
	import {
		CountColumnSide,
		isDeltaLine,
		SectionType,
		type DependencyLock,
		type DiffFileLineId,
		type Row
	} from '$lib/utils/diffParsing';
	import type LineSelection from '$lib/hunkDiff/lineSelection.svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		idx: number;
		row: Row;
		clickable?: boolean;
		clearLineSelection?: () => void;
		lineSelection: LineSelection;
		tabSize: number;
		wrapText: boolean;
		diffFont?: string;
		numberHeaderWidth?: number;
		onQuoteSelection?: () => void;
		onCopySelection?: () => void;
		hoveringOverTable: boolean;
		staged?: boolean;
		hideCheckboxes?: boolean;
		handleLineContextMenu?: (params: ContextMenuParams) => void;
		minWidth: number;
		lockWarning?: Snippet<[DependencyLock[]]>;
	}

	const {
		idx,
		row,
		clickable: isClickable = false,
		lineSelection,
		tabSize,
		wrapText,
		diffFont = 'var(--fontfamily-mono)',
		clearLineSelection,
		numberHeaderWidth,
		onQuoteSelection,
		onCopySelection,
		hoveringOverTable,
		staged,
		hideCheckboxes,
		handleLineContextMenu,
		minWidth,
		lockWarning
	}: Props = $props();

	const touchDevice = isTouchDevice();

	let rowElement = $state<HTMLTableRowElement>();
	let overflowMenuHeight = $state<number>(0);
	let stagingColumnWidth = $state<number>(0);

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

	const locked = $derived(row.locks !== undefined && row.locks.length > 0);
	const clickable = $derived(isClickable && !locked);
</script>

{#snippet countColumn(side: CountColumnSide)}
	{@const deltaLine = isDeltaLine(row.type)}
	<td
		class="table__numberColumn"
		data-no-drag
		class:diff-line-deletion={row.type === SectionType.RemovedLines}
		class:diff-line-addition={row.type === SectionType.AddedLines}
		class:clickable
		align="center"
		class:is-last={row.isLast}
		class:is-before={side === CountColumnSide.Before}
		class:staged={staged && deltaLine}
		class:locked
		style="--staging-column-width: {stagingColumnWidth}px; --number-col-width: {minWidth}rem;"
		class:stagable={staged !== undefined && !hideCheckboxes}
		onmousedown={(ev) => !locked && lineSelection.onStart(ev, row, idx)}
		onmouseenter={(ev) => !locked && lineSelection.onMoveOver(ev, row, idx)}
		onmouseup={() => !locked && lineSelection.onEnd()}
		oncontextmenu={(ev) => {
			ev.preventDefault();
			ev.stopPropagation();
			handleLineContextMenu?.({
				event: ev,
				beforeLineNumber: row.beforeLineNumber,
				afterLineNumber: row.afterLineNumber
			});
		}}
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
	{#if staged !== undefined && !hideCheckboxes}
		{@const deltaLine = isDeltaLine(row.type)}
		<td
			bind:clientWidth={stagingColumnWidth}
			class="table__numberColumn"
			style="--staging-column-width: {stagingColumnWidth}px; --number-col-width: {minWidth}rem;"
			data-no-drag
			class:diff-line-deletion={row.type === SectionType.RemovedLines}
			class:diff-line-addition={row.type === SectionType.AddedLines}
			class:clickable
			align="center"
			class:is-last={row.isLast}
			class:staged={staged && deltaLine}
			class:locked
			onmousedown={(ev) => !locked && lineSelection.onStart(ev, row, idx)}
			onmouseenter={(ev) => !locked && lineSelection.onMoveOver(ev, row, idx)}
			onmouseup={() => !locked && lineSelection.onEnd()}
			oncontextmenu={(ev) => {
				ev.preventDefault();
				ev.stopPropagation();
				handleLineContextMenu?.({
					event: ev,
					beforeLineNumber: row.beforeLineNumber,
					afterLineNumber: row.afterLineNumber
				});
			}}
		>
			{#if deltaLine}
				<div class="table__row-checkbox" class:locked>
					{#if locked}
						{@const locks = row.locks}
						{#if lockWarning && locks && locks.length > 0}
							<div class="table__row-locks-info-button">
								<InfoButton inheritColor size="small" icon="locked-small">
									{@render lockWarning(locks)}
								</InfoButton>
							</div>
						{:else}
							<Icon name="locked-small" />
						{/if}
					{:else if staged}
						<Checkbox checked={staged} small style="ghost" />
					{:else}
						<Icon name="minus-small" />
					{/if}
				</div>
			{/if}
		</td>
	{/if}

	{@render countColumn(CountColumnSide.Before)}
	{@render countColumn(CountColumnSide.After)}
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
		oncontextmenu={(ev) => {
			ev.preventDefault();
			ev.stopPropagation();
			handleLineContextMenu?.({
				event: ev,
				beforeLineNumber: row.beforeLineNumber,
				afterLineNumber: row.afterLineNumber
			});
		}}
	>
		<div data-no-drag class="table__row-header">
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
	}

	.table__row-header {
		min-height: 20px;
		white-space: pre;
		user-select: text;
		-webkit-user-select: text;
		cursor: text;
		position: relative;
		text-wrap: var(--wrap);
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
		opacity: 0;
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
		z-index: var(--z-ground);
		color: var(--clr-diff-count-text);
		border-color: var(--clr-diff-count-border);
		background-color: var(--clr-diff-count-bg);
		font-size: 11px;
		text-align: center;
		padding: 0 4px;
		text-align: right;
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
			--checkmark-color: var(--clr-diff-addition-count-checkmark);
		}

		&.diff-line-deletion {
			background-color: var(--clr-diff-deletion-count-bg);
			color: var(--clr-diff-deletion-count-text);
			border-color: var(--clr-diff-deletion-count-border);
			--checkmark-color: var(--clr-diff-deletion-count-checkmark);
		}

		&.is-before.is-last {
			border-bottom-left-radius: var(--radius-s);
		}

		&.clickable {
			cursor: pointer;
		}

		/* Staging column width + 1 border width-ish. */
		/* It's kind of a hack to ad the fraction of a pixel here, but table CSS sucks */
		--column-and-boder: calc(var(--staging-column-width) + 0.5px);
		&.stagable {
			min-width: var(--staging-column-width);
			left: calc(var(--column-and-boder));
		}

		&.stagable:not(.is-before) {
			min-width: var(--staging-column-width);
			left: calc(var(--column-and-boder) * 2);
		}

		&.staged {
			background-color: var(--clr-diff-selected-count-bg);
			border-color: var(--clr-diff-selected-count-border);
			color: var(--clr-diff-selected-count-text);
		}

		&.locked {
			background-color: var(--clr-diff-locked-count-bg);
			border-color: var(--clr-diff-locked-count-border);
			color: var(--clr-diff-locked-count-text);
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

	.table__row-checkbox {
		display: flex;
		justify-content: center;
		align-items: center;
		box-sizing: border-box;
		flex-shrink: 0;
		pointer-events: none;

		color: var(--checkmark-color);
		margin: 0;
		padding: 0;
		width: 18px;
		height: 18px;

		&.locked {
			color: var(--clr-diff-locked-count-text);
		}
	}

	.table__row-locks-info-button {
		pointer-events: all;
	}
</style>
