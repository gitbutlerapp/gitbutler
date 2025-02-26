<script lang="ts" module>
	import { type LineSelectionParams } from '$lib/hunkDiff/lineSelection.svelte';
	export type LineClickParams = LineSelectionParams;
</script>

<script lang="ts">
	import Checkbox from './Checkbox.svelte';
	import HunkDiffBody from './hunkDiff/HunkDiffBody.svelte';
	import {
		type ContentSection,
		getHunkLineInfo,
		type LineSelector,
		parseHunk
	} from '$lib/utils/diffParsing';
	interface Props {
		filePath: string;
		hunkStr: string;
		tabSize?: number;
		wrapText?: boolean;
		diffFont?: string;
		diffLigatures?: boolean;
		inlineUnifiedDiffs?: boolean;
		diffContrast?: 'light' | 'medium' | 'strong';
		selected?: boolean;
		onchange?: (selected: boolean) => void;
		selectedLines?: LineSelector[];
		onLineClick?: (params: LineSelectionParams) => void;
		clearLineSelection?: (fileName: string) => void;
		onQuoteSelection?: () => void;
		onCopySelection?: (contentSections: ContentSection[]) => void;
	}

	const {
		filePath,
		hunkStr,
		tabSize = 4,
		wrapText = true,
		diffFont,
		diffLigatures = true,
		diffContrast = 'medium',
		inlineUnifiedDiffs = false,
		selected,
		onchange,
		selectedLines,
		onLineClick,
		clearLineSelection,
		onCopySelection,
		onQuoteSelection
	}: Props = $props();

	const BORDER_WIDTH = 1;

	let tableWidth = $state<number>(0);
	let tableHeight = $state<number>(0);
	let numberHeaderWidth = $state<number>(0);

	const hunk = $derived(parseHunk(hunkStr));
	const hunkLineInfo = $derived(getHunkLineInfo(hunk.contentSections));

	function handleCopySelection() {
		onCopySelection?.(hunk.contentSections);
	}
</script>

<div
	bind:clientWidth={tableWidth}
	bind:clientHeight={tableHeight}
	class="table__wrapper hide-native-scrollbar contrast-{diffContrast}"
	style="--tab-size: {tabSize}; --diff-font: {diffFont};"
	style:font-variant-ligatures={diffLigatures ? 'common-ligatures' : 'none'}
>
	<table class="table__section">
		<thead class="table__title">
			<tr>
				<th
					bind:clientWidth={numberHeaderWidth}
					class="table__checkbox-container"
					style="--border-width: {BORDER_WIDTH}px;"
					colspan={2}
				>
					<div class="table__checkbox">
						{#if selected !== undefined}
							<Checkbox
								checked={selected}
								small
								onchange={(e) => {
									onchange?.(e.currentTarget.checked);
								}}
							/>
						{/if}
					</div>
					<div
						class="table__title-content"
						style="--number-col-width: {numberHeaderWidth}px; --table-width: {tableWidth}px; --border-width: {BORDER_WIDTH}px; --top: -{BORDER_WIDTH}px"
					>
						<span>
							{`@@ -${hunkLineInfo.beforLineStart},${hunkLineInfo.beforeLineCount} +${hunkLineInfo.afterLineStart},${hunkLineInfo.afterLineCount} @@`}
						</span>
					</div>
				</th>
			</tr>
		</thead>

		<HunkDiffBody
			{filePath}
			content={hunk.contentSections}
			{onLineClick}
			clearLineSelection={() => clearLineSelection?.(filePath)}
			{wrapText}
			{tabSize}
			{inlineUnifiedDiffs}
			{selectedLines}
			{diffContrast}
			{numberHeaderWidth}
			onCopySelection={onCopySelection && handleCopySelection}
			{onQuoteSelection}
		/>
	</table>
</div>

<style lang="postcss">
	.table__wrapper {
		border-radius: var(--radius-s);
		background-color: var(--clr-diff-line-bg);
		border: 1px solid var(--clr-border-2);
		overflow-x: auto;
		width: 100%;
	}

	table,
	.table__section {
		width: 100%;
		font-family: var(--diff-font);
		border-collapse: collapse;
		border-spacing: 0;
		user-select: none;
	}

	thead {
		width: 100%;
		padding: 0;
	}

	th,
	tr {
		padding: 0;
		margin: 0;
	}

	table thead th {
		top: 0;
		left: 0;
		position: sticky;
		height: 28px;
	}

	.table__checkbox-container {
		z-index: var(--z-lifted);

		border-right: 1px solid var(--clr-border-2);
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-diff-count-bg);
		border-top-left-radius: var(--radius-s);
		box-sizing: border-box;

		&.selected {
			background-color: var(--clr-diff-selected-count-bg);
			border-color: var(--clr-diff-selected-count-border);
		}
	}

	.table__checkbox {
		padding: 4px 6px;
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.table__title {
		user-select: none;
	}

	.draggable {
		cursor: grab;
	}

	.table__drag-handle {
		position: fixed;
		right: 6px;
		top: 6px;
		box-sizing: border-box;
		background-color: var(--clr-bg-1);
		display: flex;
		justify-content: center;
		align-items: center;
		border-radius: var(--radius-s);
		opacity: 0;
		transform: scale(0.9);
		transform-origin: top right;
		pointer-events: none;
		color: var(--clr-text-2);
		transition:
			opacity 0.2s,
			transform 0.2s;
	}

	.table__lock {
		position: fixed;
		right: 6px;
		top: 6px;
		box-sizing: border-box;
		background-color: var(--clr-theme-warn-soft);
		display: flex;
		justify-content: center;
		align-items: center;
		border-radius: var(--radius-s);
		pointer-events: none;
		color: var(--clr-text-2);
		transition: transform var(--transition-medium);
	}

	.table__title-content {
		color: var(--clr-text-2, #867e79);
		font-family: 'Geist Mono';
		font-size: 12px;
		font-style: normal;
		font-weight: 400;
		line-height: 120%; /* 14.4px */

		position: absolute;
		top: var(--top);
		left: var(--number-col-width);
		width: calc(var(--table-width) - var(--number-col-width));
		height: calc(100% + var(--border-width) * 2);
		box-sizing: border-box;
		padding: 4px 6px;
		text-wrap: nowrap;

		display: flex;
		align-items: center;
		border-bottom: 1px solid var(--clr-border-2);
		border-top-right-radius: var(--radius-m);
	}
</style>
