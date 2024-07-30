<script lang="ts">
	import { type Row, Operation, type DiffRows } from './types';
	import Icon from '$lib/shared/Icon.svelte';
	import Scrollbar from '$lib/shared/Scrollbar.svelte';
	import { create } from '$lib/utils/codeHighlight';
	import { maybeGetContextStore } from '$lib/utils/context';
	import { type ContentSection, SectionType, type Line } from '$lib/utils/fileSections';
	import { Ownership } from '$lib/vbranches/ownership';
	import { type Hunk } from '$lib/vbranches/types';
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

	let viewport = $state<HTMLDivElement>();
	let contents = $state<HTMLDivElement>();

	const WHITESPACE_REGEX = /\s/;
	const NUMBER_COLUMN_WIDTH_PX = minWidth * 20;

	const selectedOwnership: Writable<Ownership> | undefined = maybeGetContextStore(Ownership);

	const selected = $derived($selectedOwnership?.contains(hunk.filePath, hunk.id) ?? false);
	let isSelected = $derived(selectable && selected);

	function charDiff(text1: string, text2: string): { 0: number; 1: string }[] {
		const differ = new diff_match_patch();
		const diff = differ.diff_main(text1, text2);
		differ.diff_cleanupSemantic(diff);
		return diff;
	}

	function isLineEmpty(lines: Line[]) {
		const whitespaceRegex = new RegExp(WHITESPACE_REGEX);
		if (!lines[0].content.match(whitespaceRegex)) {
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
				size: line.content.length
			};
		});
	}

	function toTokens(inputLine: string): string[] {
		function sanitize(text: string) {
			var element = document.createElement('div');
			element.innerText = text;
			return element.innerHTML;
		}

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
			const oldLine = prevSection.lines[i];
			const newLine = nextSection.lines[i];
			const prevSectionRow = {
				beforeLineNumber: oldLine.beforeLineNumber,
				afterLineNumber: oldLine.afterLineNumber,
				tokens: [] as string[],
				type: prevSection.sectionType,
				size: oldLine.content.length
			};
			const nextSectionRow = {
				beforeLineNumber: newLine.beforeLineNumber,
				afterLineNumber: newLine.afterLineNumber,
				tokens: [] as string[],
				type: nextSection.sectionType,
				size: newLine.content.length
			};

			const diff = charDiff(oldLine.content, newLine.content);

			for (const token of diff) {
				const text = token[1];
				const type = token[0];

				if (type === Operation.Equal) {
					prevSectionRow.tokens.push(...toTokens(text));
					nextSectionRow.tokens.push(...toTokens(text));
				} else if (type === Operation.Insert) {
					nextSectionRow.tokens.push(`<span data-no-drag class="token-inserted">${text}</span>`);
				} else if (type === Operation.Delete) {
					prevSectionRow.tokens.push(`<span data-no-drag class="token-deleted">${text}</span>`);
				}
			}
			returnRows.nextRows.push(nextSectionRow);
			returnRows.prevRows.push(prevSectionRow);
		}

		return returnRows;
	}

	function generateRows(subsections: ContentSection[]) {
		return subsections.reduce((acc, nextSection, i) => {
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

			const { prevRows, nextRows } = computeWordDiff(prevSection, nextSection);

			// Insert returned row datastructures into the correct place
			// Find and replace previous rows with tokenized version
			prevRows.forEach((row) => {
				const accIndex = acc.findIndex(
					(accRow) =>
						accRow.beforeLineNumber === row.beforeLineNumber &&
						accRow.afterLineNumber === row.afterLineNumber
				);
				if (!accIndex) return;

				acc[accIndex] = row;
			});

			acc.push(...nextRows);

			return acc;
		}, [] as Row[]);
	}

	const renderRows = $derived(generateRows(subsections));
</script>

{#snippet countColumn(count: number | undefined, lineType: SectionType)}
	<td
		class="table__numberColumn"
		class:diff-line-deletion={lineType === SectionType.RemovedLines}
		class:diff-line-addition={lineType === SectionType.AddedLines}
		style="--number-col-width: {NUMBER_COLUMN_WIDTH_PX}px;"
		align="center"
		class:selected={isSelected}
		onclick={() => {
			selectable && handleSelected(hunk, !isSelected);
		}}
	>
		{count}
	</td>
{/snippet}

<div
	class="table__wrapper hide-native-scrollbar"
	bind:this={viewport}
	style="--tab-size: {tabSize}; --cursor: {draggingDisabled ? 'default' : 'grab'}"
>
	{#if !draggingDisabled}
		<div class="table__drag-handle">
			<Icon name="draggable-narrow" />
		</div>
	{/if}
	<table bind:this={contents} data-hunk-id={hunk.id} class="table__section">
		<tbody>
			{#each renderRows as line}
				<tr data-no-drag>
					{@render countColumn(line.beforeLineNumber, line.type)}
					{@render countColumn(line.afterLineNumber, line.type)}
					<td
						{onclick}
						class="table__textContent"
						style="--tab-size: {tabSize};"
						class:readonly
						data-no-drag
						class:diff-line-deletion={line.type === SectionType.RemovedLines}
						class:diff-line-addition={line.type === SectionType.AddedLines}
						oncontextmenu={(event) => {
							const lineNumber = (line.beforeLineNumber
								? line.beforeLineNumber
								: line.afterLineNumber) as number;
							handleLineContextMenu({ event, hunk, lineNumber, subsection: subsections[0] });
						}}
					>
						{@html line.tokens.join('')}
					</td>
				</tr>
			{/each}
		</tbody>
	</table>
	<Scrollbar {viewport} {contents} horz padding={{ left: NUMBER_COLUMN_WIDTH_PX * 2 + 2 }} />
</div>

<style>
	.table__wrapper {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-s);
		background-color: var(--clr-bg-1);
		overflow-x: auto;

		&:hover .table__drag-handle {
			transform: translateY(0) translateX(0) scale(1);
			opacity: 1;
			pointer-events: auto;
		}
	}

	.table__drag-handle {
		position: absolute;
		cursor: grab;
		top: 6px;
		right: 6px;
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		display: flex;
		justify-content: center;
		align-items: center;
		padding: 4px 2px;
		border-radius: var(--radius-s);
		opacity: 0;
		transform: translateY(10%) translateX(-10%) scale(0.9);
		transform-origin: top right;
		pointer-events: none;
		transition:
			opacity 0.2s,
			transform 0.2s;
	}

	.table__section {
		border-spacing: 0;
		width: 100%;
		font-family: monospace;
	}

	.table__numberColumn {
		color: color-mix(in srgb, var(--clr-text-1), transparent 60%);
		border-color: var(--clr-border-2);
		background-color: var(--clr-bg-1-muted);
		font-size: 11px;
		text-align: center;
		padding: 0 4px;
		text-align: right;
		cursor: var(--cursor);
		user-select: none;

		position: sticky;
		left: calc(var(--number-col-width));
		width: var(--number-col-width);
		min-width: var(--number-col-width);

		box-shadow: inset -1px 0 0 0 var(--clr-border-2);

		&.diff-line-addition {
			background-color: var(--override-addition-counter-background);
			color: var(--override-addition-counter-text);
			box-shadow: inset -1px 0 0 0 var(--override-addition-counter-border);
		}

		&.diff-line-deletion {
			background-color: var(--override-deletion-counter-background);
			color: var(--override-deletion-counter-text);
			box-shadow: inset -1px 0 0 0 var(--override-deletion-counter-border);
		}

		&.selected {
			background-color: var(--hunk-line-selected-bg);
			box-shadow: inset -1px 0 0 0 var(--hunk-line-selected-border);
			color: rgba(255, 255, 255, 0.9);
		}
	}

	.table__numberColumn:first-of-type {
		width: var(--number-col-width);
		min-width: var(--number-col-width);
		left: 0px;
	}

	.table__textContent {
		width: 100%;
		font-size: 12px;
		padding-left: 4px;
		line-height: 1.25;
		tab-size: var(--tab-size);
		white-space: pre;
		user-select: text;

		&:hover {
			cursor: text;
		}
	}
</style>
