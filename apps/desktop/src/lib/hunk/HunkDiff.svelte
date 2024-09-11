<script lang="ts">
	import { type Row, Operation, type DiffRows } from './types';
	import { featureInlineUnifiedDiffs } from '$lib/config/uiFeatureFlags';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import { create } from '$lib/utils/codeHighlight';
	import { maybeGetContextStore } from '$lib/utils/context';
	import {
		type ContentSection,
		SectionType,
		type Line,
		CountColumnSide
	} from '$lib/utils/fileSections';
	import { SelectedOwnership } from '$lib/vbranches/ownership';
	import { type Hunk } from '$lib/vbranches/types';
	import Checkbox from '@gitbutler/ui/Checkbox.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import diff_match_patch from 'diff-match-patch';
	import type { Writable } from 'svelte/store';

	interface Props {
		hunk: Hunk;
		readonly: boolean;
		filePath: string;
		selectable: boolean;
		subsections: ContentSection[];
		tabSize: number;
		minWidth: number;
		draggingDisabled: boolean;
		onclick: () => void;
		handleSelected: (hunk: Hunk, isSelected: boolean) => void;
		handleLineContextMenu: ({
			event,
			lineNumber,
			hunk,
			subsection
		}: {
			event: MouseEvent;
			lineNumber: number;
			hunk: Hunk;
			subsection: ContentSection;
		}) => void;
	}

	const {
		hunk,
		readonly = false,
		filePath,
		selectable,
		subsections,
		tabSize,
		minWidth,
		draggingDisabled = false,
		onclick,
		handleSelected,
		handleLineContextMenu
	}: Props = $props();

	const WHITESPACE_REGEX = /\s/;
	const NUMBER_COLUMN_WIDTH_PX = minWidth * 20;

	const selectedOwnership: Writable<SelectedOwnership> | undefined =
		maybeGetContextStore(SelectedOwnership);

	let tableWidth = $state<number>(0);

	const selected = $derived($selectedOwnership?.isSelected(hunk.filePath, hunk.id) ?? false);
	let isSelected = $derived(selectable && selected);

	const inlineUnifiedDiffs = featureInlineUnifiedDiffs();

	function charDiff(text1: string, text2: string): { 0: number; 1: string }[] {
		const differ = new diff_match_patch();
		const diff = differ.diff_main(text1, text2);
		differ.diff_cleanupSemantic(diff);
		return diff;
	}

	function isLineEmpty(lines: Line[]) {
		const whitespaceRegex = new RegExp(WHITESPACE_REGEX);
		if (!lines[0]?.content.match(whitespaceRegex)) {
			return true;
		}

		return false;
	}

	function createRowData(section: ContentSection): Row[] {
		return section.lines.map((line) => {
			if (line.content === '') {
				// Add extra \n for empty lines for correct copy/pasting output
				line.content = '\n';
			}

			return {
				beforeLineNumber: line.beforeLineNumber,
				afterLineNumber: line.afterLineNumber,
				tokens: toTokens(line.content),
				type: section.sectionType,
				size: line.content.length,
				isLast: false
			};
		});
	}

	function sanitize(text: string) {
		const element = document.createElement('div');
		element.innerText = text;
		return element.innerHTML;
	}

	function toTokens(inputLine: string): string[] {
		let highlighter = create(inputLine, filePath);
		let tokens: string[] = [];
		highlighter.highlight((text, classNames) => {
			const token = classNames
				? `<span data-no-drag class=${classNames}>${sanitize(text)}</span>`
				: sanitize(text);

			tokens.push(token);
		});

		return tokens;
	}

	function computeWordDiff(prevSection: ContentSection, nextSection: ContentSection): DiffRows {
		const numberOfLines = nextSection.lines.length;
		const returnRows: DiffRows = {
			prevRows: [],
			nextRows: []
		};

		// Loop through every line in the section
		// We're only bothered with prev/next sections with equal # of lines changes
		for (let i = 0; i < numberOfLines; i++) {
			const oldLine = prevSection.lines[i] as Line;
			const newLine = nextSection.lines[i] as Line;
			const prevSectionRow = {
				beforeLineNumber: oldLine.beforeLineNumber,
				afterLineNumber: oldLine.afterLineNumber,
				tokens: [] as string[],
				type: prevSection.sectionType,
				size: oldLine.content.length,
				isLast: false
			};
			const nextSectionRow = {
				beforeLineNumber: newLine.beforeLineNumber,
				afterLineNumber: newLine.afterLineNumber,
				tokens: [] as string[],
				type: nextSection.sectionType,
				size: newLine.content.length,
				isLast: false
			};

			const diff = charDiff(oldLine.content, newLine.content);

			for (const token of diff) {
				const text = token[1];
				const type = token[0];

				if (type === Operation.Equal) {
					prevSectionRow.tokens.push(...toTokens(text));
					nextSectionRow.tokens.push(...toTokens(text));
				} else if (type === Operation.Insert) {
					nextSectionRow.tokens.push(
						`<span data-no-drag class="token-inserted">${sanitize(text)}</span>`
					);
				} else if (type === Operation.Delete) {
					prevSectionRow.tokens.push(
						`<span data-no-drag class="token-deleted">${sanitize(text)}</span>`
					);
				}
			}
			returnRows.nextRows.push(nextSectionRow);
			returnRows.prevRows.push(prevSectionRow);
		}

		return returnRows;
	}

	function computeInlineWordDiff(prevSection: ContentSection, nextSection: ContentSection): Row[] {
		const numberOfLines = nextSection.lines.length;

		const rows = [];

		// Loop through every line in the section
		// We're only bothered with prev/next sections with equal # of lines changes
		for (let i = 0; i < numberOfLines; i++) {
			const oldLine = prevSection.lines[i] as Line;
			const newLine = nextSection.lines[i] as Line;

			const sectionRow = {
				beforeLineNumber: newLine.beforeLineNumber,
				afterLineNumber: newLine.afterLineNumber,
				tokens: [] as string[],
				type: nextSection.sectionType,
				size: newLine.content.length,
				isLast: false
			};

			const diff = charDiff(oldLine.content, newLine.content);

			for (const token of diff) {
				const text = token[1];
				const type = token[0];

				if (type === Operation.Equal) {
					sectionRow.tokens.push(...toTokens(text));
				} else if (type === Operation.Insert) {
					sectionRow.tokens.push(
						`<span data-no-drag class="token-inserted">${sanitize(text)}</span>`
					);
				} else if (type === Operation.Delete) {
					sectionRow.tokens.push(
						`<span data-no-drag class="token-deleted token-strikethrough">${sanitize(text)}</span>`
					);
				}
			}
			rows.push(sectionRow);
		}

		return rows;
	}

	function generateRows(subsections: ContentSection[]) {
		const rows = subsections.reduce((acc, nextSection, i) => {
			const prevSection = subsections[i - 1];

			// Filter out section for which we don't need to compute word diffs
			if (!prevSection || nextSection.sectionType === SectionType.Context) {
				acc.push(...createRowData(nextSection));
				return acc;
			}

			if (prevSection.sectionType === SectionType.Context) {
				acc.push(...createRowData(nextSection));
				return acc;
			}

			if (prevSection.lines.length !== nextSection.lines.length) {
				acc.push(...createRowData(nextSection));
				return acc;
			}

			if (isLineEmpty(prevSection.lines)) {
				acc.push(...createRowData(nextSection));
				return acc;
			}

			if ($inlineUnifiedDiffs) {
				const rows = computeInlineWordDiff(prevSection, nextSection);

				acc.splice(-prevSection.lines.length);

				acc.push(...rows);
				return acc;
			} else {
				const { prevRows, nextRows } = computeWordDiff(prevSection, nextSection);

				// Insert returned row datastructures into the correct place
				// Find and replace previous rows with tokenized version
				prevRows.forEach((row, previousRowIndex) => {
					acc[acc.length - (prevRows.length - previousRowIndex)] = row;
				});

				acc.push(...nextRows);

				return acc;
			}
		}, [] as Row[]);

		const last = rows.at(-1);
		if (last) {
			last.isLast = true;
		}

		return rows;
	}

	const renderRows = $derived(generateRows(subsections));

	interface DiffHunkLineInfo {
		beforLineStart: number;
		beforeLineCount: number;
		afterLineStart: number;
		afterLineCount: number;
	}

	function getHunkLineInfo(subsections: ContentSection[]): DiffHunkLineInfo {
		const firstSection = subsections[0];
		const lastSection = subsections.at(-1);

		const beforLineStart = firstSection?.lines[0]?.beforeLineNumber ?? 0;
		const beforeLineEnd = lastSection?.lines?.at(-1)?.beforeLineNumber ?? 0;
		const beforeLineCount = beforeLineEnd - beforLineStart + 1;

		const afterLineStart = firstSection?.lines[0]?.afterLineNumber ?? 0;
		const afterLineEnd = lastSection?.lines?.at(-1)?.afterLineNumber ?? 0;
		const afterLineCount = afterLineEnd - afterLineStart + 1;

		return {
			beforLineStart,
			beforeLineCount,
			afterLineStart,
			afterLineCount
		};
	}

	const hunkLineInfo = $derived(getHunkLineInfo(subsections));
</script>

{#snippet countColumn(row: Row, side: CountColumnSide)}
	<td
		class="table__numberColumn"
		data-no-drag
		class:diff-line-deletion={row.type === SectionType.RemovedLines}
		class:diff-line-addition={row.type === SectionType.AddedLines}
		style="--number-col-width: {NUMBER_COLUMN_WIDTH_PX + 2}px;"
		align="center"
		class:is-last={row.isLast}
		class:is-before={side === CountColumnSide.Before}
		class:selected={isSelected}
		onclick={() => {
			selectable && handleSelected(hunk, !isSelected);
		}}
	>
		{side === CountColumnSide.Before ? row.beforeLineNumber : row.afterLineNumber}
	</td>
{/snippet}

<div
	bind:clientWidth={tableWidth}
	class="table__wrapper hide-native-scrollbar"
	style="--tab-size: {tabSize}"
>
	<ScrollableContainer horz padding={{ left: NUMBER_COLUMN_WIDTH_PX * 2 + 2 }}>
		<table data-hunk-id={hunk.id} class="table__section">
			<thead class="table__title">
				<tr
					onclick={() => {
						selectable && handleSelected(hunk, !isSelected);
					}}
				>
					<th class="table__checkbox-container" class:selected={isSelected} colspan={2}>
						<div class="table__checkbox">
							{#if selectable}
								<Checkbox
									checked={isSelected}
									small
									onclick={() => {
										selectable && handleSelected(hunk, !isSelected);
									}}
								/>
							{/if}
						</div>
					</th>

					<td class="table__title-content">
						<span style="left: {NUMBER_COLUMN_WIDTH_PX * 2}px">
							{`@@ -${hunkLineInfo.beforLineStart},${hunkLineInfo.beforeLineCount} +${hunkLineInfo.afterLineStart},${hunkLineInfo.afterLineCount} @@`}
						</span>
						{#if !draggingDisabled}
							<div class="table__drag-handle">
								<Icon name="draggable" />
							</div>
						{/if}
					</td>
				</tr>
			</thead>

			<tbody>
				{#each renderRows as row}
					<tr data-no-drag>
						{@render countColumn(row, CountColumnSide.Before)}
						{@render countColumn(row, CountColumnSide.After)}
						<td
							{onclick}
							class="table__textContent"
							style="--tab-size: {tabSize};"
							class:readonly
							data-no-drag
							class:diff-line-deletion={row.type === SectionType.RemovedLines}
							class:diff-line-addition={row.type === SectionType.AddedLines}
							class:is-last={row.isLast}
							oncontextmenu={(event) => {
							const lineNumber = (row.beforeLineNumber
								? row.beforeLineNumber
								: row.afterLineNumber) as number;
							handleLineContextMenu({ event, hunk, lineNumber, subsection: subsections[0] as ContentSection });
						}}
						>
							{@html row.tokens.join('')}
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</ScrollableContainer>
</div>

<style lang="postcss">
	.table__wrapper {
		border-radius: var(--radius-m);
		background-color: var(--clr-diff-line-bg);
		overflow-x: auto;
		border: 1px solid var(--clr-border-2);

		&:hover .table__drag-handle {
			transform: scale(1);
			opacity: 1;
		}
	}

	table,
	.table__section {
		width: 100%;
		font-family: var(--mono-font-family);
		border-collapse: separate;
		border-spacing: 0;
	}

	thead {
		width: 100%;
		padding: 0;
	}

	th,
	td,
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
			border-right: 1px solid var(--clr-diff-selected-count-border);
			border-bottom: 1px solid var(--clr-diff-selected-count-border);
		}
	}

	.table__checkbox {
		padding: 4px 6px;
		display: flex;
		align-items: center;
	}

	.table__title {
		cursor: grab;
		user-select: none;
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

	.table__title-content {
		position: relative;
		font-family: var(--mono-font-family);
		font-size: 12px;
		padding: 4px 6px;
		text-wrap: nowrap;
		color: var(--clr-text-2);
		border-bottom: 1px solid var(--clr-border-2);
	}

	.table__numberColumn {
		color: var(--clr-diff-count-text);
		border-color: var(--clr-diff-count-border);
		background-color: var(--clr-diff-count-bg);
		font-size: 11px;
		text-align: center;
		padding: 0 4px;
		text-align: right;
		user-select: none;

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

		&.selected {
			background-color: var(--clr-diff-selected-count-bg);
			color: var(--clr-diff-selected-count-text);
			border-color: var(--clr-diff-selected-count-border);
		}

		&.is-last {
			border-bottom-width: 1px;
		}

		&.is-before.is-last {
			border-bottom-left-radius: var(--radius-s);
		}
	}

	.table__numberColumn:first-of-type {
		width: var(--number-col-width);
		min-width: var(--number-col-width);
		left: 0px;
	}

	.table__textContent {
		z-index: var(--z-lifted);
		width: 100%;
		font-size: 12px;
		padding-left: 4px;
		line-height: 1.25;
		tab-size: var(--tab-size);
		white-space: pre;
		user-select: text;
		cursor: text;
	}
</style>
