<script lang="ts" module>
	import { type LineSelectionParams } from '$lib/hunkDiff/lineSelection.svelte';
	export type LineClickParams = LineSelectionParams;
</script>

<script lang="ts">
	import Button from './Button.svelte';
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
		selectedLines?: LineSelector[];
		isHidden?: boolean;
		whyHidden?: string;
		onShowDiffClick?: () => void;
		onchange?: (selected: boolean) => void;
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
		diffFont = 'var(--fontfamily-mono)',
		diffLigatures = true,
		diffContrast = 'medium',
		inlineUnifiedDiffs = false,
		selected,
		selectedLines,
		isHidden,
		whyHidden,
		onShowDiffClick,
		onchange,
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

	const hunkSummary = $derived(
		`@@ -${hunkLineInfo.beforLineStart},${hunkLineInfo.beforeLineCount} +${hunkLineInfo.afterLineStart},${hunkLineInfo.afterLineCount} @@`
	);
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
							{hunkSummary}
						</span>
					</div>
				</th>
			</tr>
		</thead>

		{#if isHidden}
			<tbody class="table__hiddenRows">
				<tr>
					<td class="table__hiddenRows__count"></td>
					<td class="table__hiddenRows__count"></td>
					<td>
						<div class="table__hiddenRows__content">
							<p class="text-12 table__hiddenRows__caption">
								{#if whyHidden}
									{whyHidden}
								{:else}
									Diff is too large to display
								{/if}
							</p>
							<Button kind="outline" onclick={onShowDiffClick} icon="eye-shown">Show anyway</Button>
						</div>
					</td>
				</tr>
			</tbody>
		{:else}
			<HunkDiffBody
				{filePath}
				content={hunk.contentSections}
				{onLineClick}
				clearLineSelection={() => clearLineSelection?.(filePath)}
				{wrapText}
				{tabSize}
				{inlineUnifiedDiffs}
				{selectedLines}
				{numberHeaderWidth}
				onCopySelection={onCopySelection && handleCopySelection}
				{onQuoteSelection}
			/>
		{/if}
	</table>
</div>

<style lang="postcss">
	.table__wrapper {
		border-radius: var(--radius-m);
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
		border-right: 1px solid var(--clr-border-2);
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-diff-count-bg);
		border-top-left-radius: var(--radius-m);
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
		border-radius: var(--radius-m);
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
		border-radius: var(--radius-m);
		pointer-events: none;
		color: var(--clr-text-2);
		transition: transform var(--transition-medium);
	}

	.table__title-content {
		color: var(--clr-text-2);
		font-size: 12px;

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

	/* HIDDINE LINES STATE */

	.table__hidden-rows {
		display: none;
	}

	.table__hiddenRows__count {
		width: 24px;
		background-color: var(--clr-diff-count-bg);

		&:nth-child(2) {
			border-right: 1px solid var(--clr-border-2);
		}
	}

	.table__hiddenRows__content {
		display: flex;
		align-items: center;
		justify-content: center;
		flex-direction: column;
		gap: 14px;
		padding: 40px 24px;
		background-color: var(--clr-bg-1-muted);
	}

	.table__hiddenRows__caption {
		color: var(--clr-text-2);
		text-align: center;
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
