<script lang="ts" module>
	export type ContextMenuParams = {
		event?: MouseEvent;
		target?: HTMLElement;
		beforeLineNumber: number | undefined;
		afterLineNumber: number | undefined;
	};
</script>

<script lang="ts">
	import Icon from "$components/Icon.svelte";
	import InfoButton from "$components/InfoButton.svelte";
	import {
		CountColumnSide,
		isDeltaLine,
		SectionType,
		type DependencyLock,
		type Row,
	} from "$lib/utils/diffParsing";
	import { getHunkLineId } from "$lib/utils/hunk";
	import type LineSelection from "$components/hunkDiff/lineSelection.svelte";
	import type { Snippet } from "svelte";

	interface Props {
		idx: number;
		row: Row;
		clickable?: boolean;
		lineSelection: LineSelection;
		tabSize: number;
		wrapText: boolean;
		diffFont?: string;
		numberHeaderWidth?: number;
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
		diffFont = "var(--font-mono)",
		numberHeaderWidth,
		staged,
		hideCheckboxes,
		handleLineContextMenu,
		minWidth,
		lockWarning,
		hunkHasLocks,
	}: Props = $props();

	let stagingColumnWidth = $state<number>(0);

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
				afterLineNumber: row.afterLineNumber,
			});
		}}
	>
		{side === CountColumnSide.Before ? row.beforeLineNumber : row.afterLineNumber}
	</td>
{/snippet}

<tr
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
					afterLineNumber: row.afterLineNumber,
				});
			}}
		>
			{#if deltaLine}
				<div class="table__row-checkbox" class:staged class:locked>
					{#if staged}
						<Icon name="tick" />
					{:else}
						<Icon name="minus" />
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
				<InfoButton
					inheritColor
					size="small"
					icon="lock"
					iconSize={10}
					maxWidth="15rem"
					iconTopOffset="0"
				>
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
		oncontextmenu={(ev) => {
			ev.preventDefault();
			ev.stopPropagation();
			handleLineContextMenu?.({
				event: ev,
				beforeLineNumber: row.beforeLineNumber,
				afterLineNumber: row.afterLineNumber,
			});
		}}
	>
		<div data-no-drag class="table__row-header">
			{#if row.isSelected}
				<div
					class="table__selected-row-overlay"
					class:is-first={row.isFirstOfSelectionGroup}
					class:is-last={row.isLastOfSelectionGroup}
					style="--number-col-width: {numberHeaderWidth}px; "
				></div>
			{/if}

			{@html row.tokens.join("")}
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
		width: 100%;
		height: 100%;
		border-right: 1px solid var(--fill-warn-bg);
		border-left: 1px solid var(--fill-warn-bg);
		background: color-mix(in srgb, var(--btn-warn-outline-bg), transparent 30%);
		mix-blend-mode: multiply;

		&.is-first {
			border-top: 1px solid var(--fill-warn-bg);
		}

		&.is-last {
			border-bottom: 1px solid var(--fill-warn-bg);
		}
	}

	.table__numberColumn {
		z-index: var(--z-ground);
		width: var(--number-col-width);
		min-width: var(--number-col-width);
		padding: 0 4px;
		border-color: var(--diff-count-border);
		border-right: 1px solid var(--diff-count-border);
		background-color: var(--diff-count-bg);
		color: var(--diff-count-text);
		font-size: 11px;
		line-height: 1.5; /* Visually centered with 12px font size that diff lines have */
		text-align: right;
		vertical-align: top;
		touch-action: none;
		user-select: none;

		&.diff-line-addition {
			border-color: var(--diff-addition-count-border);
			background-color: var(--diff-addition-count-bg);
			color: var(--diff-addition-count-text);
		}

		&.diff-line-deletion {
			border-color: var(--diff-deletion-count-border);
			background-color: var(--diff-deletion-count-bg);
			color: var(--diff-deletion-count-text);
		}

		&.clickable {
			cursor: pointer;
		}
		&.stagable {
			min-width: var(--staging-column-width);
		}

		&.stagable:not(.is-before) {
			min-width: var(--staging-column-width);
		}

		&.staged {
			border-color: var(--diff-selected-count-border);
			background-color: var(--diff-selected-count-bg);
			color: var(--diff-selected-count-text);
		}

		&.locked {
			border-color: var(--diff-locked-count-border);
			background-color: var(--diff-locked-count-bg);
			color: var(--diff-locked-count-text);

			&.staged {
				border-color: var(--diff-locked-selected-count-border);
				background-color: var(--diff-locked-selected-count-bg);
				color: var(--diff-locked-selected-count-text);
			}
		}
	}

	.table__lockColumn {
		padding: 0 4px;
		border-color: var(--diff-count-border);
		border-right: 1px solid var(--diff-count-border);
		background-color: var(--diff-count-bg);
		line-height: 1;
		vertical-align: top;

		&.diff-line-addition {
			border-color: var(--diff-addition-count-border);
			background-color: var(--diff-addition-count-bg);
			color: var(--diff-addition-count-text);
		}

		&.diff-line-deletion {
			border-color: var(--diff-deletion-count-border);
			background-color: var(--diff-deletion-count-bg);
			color: var(--diff-deletion-count-text);
		}

		&.locked {
			border-color: var(--diff-locked-count-border);
			background-color: var(--diff-locked-count-bg);
			color: var(--diff-locked-count-text);

			&.staged {
				border-color: var(--diff-locked-selected-count-border);
				background-color: var(--diff-locked-selected-count-bg);
				color: var(--diff-locked-selected-count-text);
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
		background-color: var(--diff-addition-line-bg);
	}

	.diff-line-deletion {
		background-color: var(--diff-deletion-line-bg);
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
			color: var(--diff-selected-count-text);
		}
		&.locked {
			color: var(--diff-locked-count-text);
		}
		&.staged.locked {
			color: var(--diff-locked-selected-count-text);
		}
	}
</style>
