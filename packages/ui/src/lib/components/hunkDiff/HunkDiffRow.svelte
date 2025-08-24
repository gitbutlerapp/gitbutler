<script lang="ts" module>
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
	import Button from '$components/Button.svelte';
	import Icon from '$components/Icon.svelte';
	import InfoButton from '$components/InfoButton.svelte';
	import { isTouchDevice } from '$lib/utils/browserAgent';
	import {
		CountColumnSide,
		isDeltaLine,
		SectionType,
		type DependencyLock,
		type DiffFileLineId,
		type Row
	} from '$lib/utils/diffParsing';
	import type LineSelection from '$components/hunkDiff/lineSelection.svelte';
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
		hunkHasLocks?: boolean;
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
		lockWarning,
		hunkHasLocks
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
	const clickable = $derived(isClickable);
	const isSelectingForCommit = $derived(staged !== undefined && !hideCheckboxes);
</script>

{#snippet countColumn(side: CountColumnSide)}
	{@const deltaLine = isDeltaLine(row.type)}
	<td
		data-testid="hunk-count-column"
		data-is-delta-line={deltaLine}
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
		class:stagable={isSelectingForCommit}
		onmousedown={(ev) => lineSelection.onStart(ev, row, idx)}
		onmouseenter={(ev) => lineSelection.onMoveOver(ev, row, idx)}
		onmouseup={() => lineSelection.onEnd()}
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
	data-test-staged={staged}
	data-no-drag
	style="--diff-font: {diffFont};"
>
	{#if isSelectingForCommit}
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
			onmousedown={(ev) => lineSelection.onStart(ev, row, idx)}
			onmouseenter={(ev) => lineSelection.onMoveOver(ev, row, idx)}
			onmouseup={() => lineSelection.onEnd()}
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
				<div class="table__row-checkbox" class:staged class:locked>
					{#if staged}
						<Icon name="tick-small" />
					{:else}
						<Icon name="minus-small" />
					{/if}
				</div>
			{/if}
		</td>
	{/if}

	<!-- LOCK COLUMN -->
	{#if !isSelectingForCommit && hunkHasLocks}
		{#if lockWarning && locked}
			<td
				data-testid="hunk-line-locking-info"
				class="table__lockColumn"
				data-no-drag
				class:locked
				class:staged
			>
				<InfoButton inheritColor size="small" icon="locked-extra-small" maxWidth="15rem">
					{@render lockWarning(row.locks ?? [])}
				</InfoButton>
			</td>
		{:else}
			<td
				class="table__lockColumn"
				data-no-drag
				class:diff-line-deletion={row.type === SectionType.RemovedLines}
				class:diff-line-addition={row.type === SectionType.AddedLines}
			>
			</td>
		{/if}
	{/if}

	{@render countColumn(CountColumnSide.Before)}
	{@render countColumn(CountColumnSide.After)}

	<td
		class="table__textContent"
		style="--tab-size: {tabSize}; --pre-wrap: {wrapText ? 'pre-wrap' : 'pre'}"
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
		margin: 0;
		padding: 0;
		font-family: var(--diff-font);
		user-select: none;
	}

	.table__textContent {
		width: 100%;
		padding-left: 4px;
		font-size: 12px;
		line-height: 1.25;
		white-space: var(--pre-wrap);
		cursor: text;
		tab-size: var(--tab-size);
		user-select: text;
	}

	.table__row-header {
		position: relative;
		min-height: 18px;
		white-space: var(--pre-wrap);
		cursor: text;
	}

	.table__selected-row-overlay {
		z-index: var(--z-lifted);
		position: absolute;
		top: 0;
		pointer-events: none;

		--offset: calc(2px + 4px + var(--number-col-width));
		box-sizing: border-box;

		left: calc(var(--offset) * -1);
		width: calc(var(--width) + 1px);
		height: var(--height);
		border-right: 1px solid var(--clr-theme-warn-element);
		border-left: 1px solid var(--clr-theme-warn-element);
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
		display: flex;
		z-index: var(--z-lifted);
		position: absolute;
		top: calc(var(--height) - var(--overflow-menu-height) - 6px);
		left: 0;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background: var(--clr-bg-1);
		box-shadow: var(--fx-shadow-s);
		opacity: 0;
		pointer-events: none;
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

	.table__lockColumn {
		padding: 0;
	}

	.table__numberColumn {
		z-index: var(--z-ground);
		width: var(--number-col-width);
		min-width: var(--number-col-width);
		padding: 0 4px;
		border-color: var(--clr-diff-count-border);
		border-right: 1px solid var(--clr-border-2);
		background-color: var(--clr-diff-count-bg);
		color: var(--clr-diff-count-text);
		font-size: 11px;
		text-align: right;
		touch-action: none;
		user-select: none;

		&.diff-line-addition {
			border-color: var(--clr-diff-addition-count-border);
			background-color: var(--clr-diff-addition-count-bg);
			color: var(--clr-diff-addition-count-text);
			--checkmark-color: var(--clr-diff-addition-count-checkmark);
		}

		&.diff-line-deletion {
			border-color: var(--clr-diff-deletion-count-border);
			background-color: var(--clr-diff-deletion-count-bg);
			color: var(--clr-diff-deletion-count-text);
			--checkmark-color: var(--clr-diff-deletion-count-checkmark);
		}

		&.is-before.is-last {
			border-bottom-left-radius: var(--radius-s);
		}

		&.clickable {
			cursor: pointer;
		}
		&.stagable {
			left: calc(var(--column-and-boder));
			min-width: var(--staging-column-width);
		}

		&.stagable:not(.is-before) {
			left: calc(var(--column-and-boder) * 2);
			min-width: var(--staging-column-width);
		}

		&.staged {
			border-color: var(--clr-diff-selected-count-border);
			background-color: var(--clr-diff-selected-count-bg);
			color: var(--clr-diff-selected-count-text);
		}

		&.locked {
			border-color: var(--clr-diff-locked-count-border);
			background-color: var(--clr-diff-locked-count-bg);
			color: var(--clr-diff-locked-count-text);

			&.staged {
				border-color: var(--clr-diff-locked-selected-count-border);
				background-color: var(--clr-diff-locked-selected-count-bg);
				color: var(--clr-diff-locked-selected-count-text);
			}
		}
	}

	.table__lockColumn {
		padding: 0 1px;
		border-color: var(--clr-diff-count-border);
		border-right: 1px solid var(--clr-border-2);
		background-color: var(--clr-diff-count-bg);
		color: var(--clr-diff-count-text);

		&.diff-line-addition {
			border-color: var(--clr-diff-addition-count-border);
			background-color: var(--clr-diff-addition-count-bg);
			color: var(--clr-diff-addition-count-text);
		}

		&.diff-line-deletion {
			border-color: var(--clr-diff-deletion-count-border);
			background-color: var(--clr-diff-deletion-count-bg);
			color: var(--clr-diff-deletion-count-text);
		}

		&.locked {
			border-color: var(--clr-diff-locked-count-border);
			background-color: var(--clr-diff-locked-count-bg);
			color: var(--clr-diff-locked-count-text);

			&.staged {
				border-color: var(--clr-diff-locked-selected-count-border);
				background-color: var(--clr-diff-locked-selected-count-bg);
				color: var(--clr-diff-locked-selected-count-text);
			}
		}
	}

	.table__numberColumn:first-of-type {
		left: 0px;
		width: var(--number-col-width);
		min-width: var(--number-col-width);
	}

	/* DIFF LINE */
	.diff-line-addition {
		background-color: var(--clr-diff-addition-line-bg);
	}

	.diff-line-deletion {
		background-color: var(--clr-diff-deletion-line-bg);
	}

	.table__row-checkbox {
		box-sizing: border-box;
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: 18px;
		height: 18px;
		margin: 0;
		padding: 0;
		pointer-events: none;

		&:not(.locked).staged {
			color: var(--clr-diff-selected-count-checkmark);
		}
		&.locked {
			color: var(--clr-diff-locked-count-checkmark);
		}
		&.staged.locked {
			color: var(--clr-diff-locked-selected-count-checkmark);
		}
	}
</style>
