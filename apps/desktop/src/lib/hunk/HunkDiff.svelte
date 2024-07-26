<script lang="ts">
	import { type Row, Operation, type DiffRows } from './types';
	import { create } from '$lib/utils/codeHighlight';
	import { maybeGetContextStore } from '$lib/utils/context';
	import { type ContentSection, SectionType, type Line } from '$lib/utils/fileSections';
	import { Ownership } from '$lib/vbranches/ownership';
	import { type Hunk } from '$lib/vbranches/types';
	import diff_match_patch from 'diff-match-patch';
	import type { Writable } from 'svelte/store';

	type HandleLineContextMenuArgs = {
		event: MouseEvent;
		lineNumber: number;
		hunk: Hunk;
		subsection: ContentSection;
	};

	interface Props {
		hunk: Hunk;
		readonly: boolean;
		filePath: string;
		selectable: boolean;
		subsections: ContentSection[];
		tabSize: number;
		draggingDisabled: boolean;
		onclick: () => void;
		handleSelected: (hunk: Hunk, isSelected: boolean) => void;
		handleLineContextMenu: ({
			event,
			lineNumber,
			hunk,
			subsection
		}: HandleLineContextMenuArgs) => void;
	}

	const {
		hunk,
		readonly = false,
		filePath,
		selectable,
		subsections,
		tabSize,
		draggingDisabled = false,
		onclick,
		handleSelected,
		handleLineContextMenu
	}: Props = $props();

	const selectedOwnership: Writable<Ownership> | undefined = maybeGetContextStore(Ownership);

	const selected = $derived($selectedOwnership?.contains(hunk.filePath, hunk.id) ?? false);
	let isSelected = $derived(selectable && selected);

	function charDiff(text1: string, text2: string): { 0: number; 1: string }[] {
		const differ = new diff_match_patch();
		const diff = differ.diff_main(text1, text2);
		// @TODO Decide on cleaned up diffs or not, see netbox `serializers_/cables.py`
		// https://github.com/google/diff-match-patch/wiki/API
		differ.diff_cleanupSemantic(diff);
		return diff;
	}

	function sanitize(text: string) {
		var element = document.createElement('div');
		element.innerText = text;
		return element.innerHTML;
	}

	function isLineEmpty(lines: Line[]) {
		const whitespaceRegex = new RegExp(/\s/);
		if (!lines[0].content.match(whitespaceRegex)) {
			return true;
		}

		return false;
	}

	function createRowData(section: ContentSection): Row[] {
		return section.lines.map((line) => {
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
					nextSectionRow.tokens.push(`<span class="token-inserted">${text}</span>`);
				} else if (type === Operation.Delete) {
					prevSectionRow.tokens.push(`<span class="token-deleted">${text}</span>`);
				}
			}
			returnRows.nextRows.push(nextSectionRow);
			returnRows.prevRows.push(prevSectionRow);
		}

		return returnRows;
	}

	// Filter out section for which we don't need to compute word diffs
	function filterRows(subsections: ContentSection[]) {
		return subsections.reduce((acc, nextSection, i) => {
			const prevSection = subsections[i - 1];
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

	const renderRows = $derived(filterRows(subsections));
</script>

<div
	class="table__wrapper"
	style="--tab-size: {tabSize}; --cursor: {draggingDisabled ? 'default' : 'grab'}"
>
	<table data-hunk-id={hunk.id} class="table__section">
		<tbody>
			{#each renderRows as line}
				<tr data-no-drag>
					<td
						class="table__numberColumn"
						align="center"
						class:selected={isSelected}
						onclick={() => {
							selectable && handleSelected(hunk, isSelected);
						}}
					>
						232{line.beforeLineNumber}
					</td>
					<td
						class="table__numberColumn"
						align="center"
						class:selected={isSelected}
						onclick={() => {
							selectable && handleSelected(hunk, isSelected);
						}}
					>
						{line.afterLineNumber}
					</td>
					<td
						{onclick}
						class="table__textContent"
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
						{@html line.tokens.join('') + '\n'}
					</td>
				</tr>
			{/each}
		</tbody>
	</table>
</div>

<style>
	.table__wrapper {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-s);
		overflow: hidden;
	}
	.table__section {
		border-spacing: 0;
		width: 100%;
		font-family: monospace;
	}

	.table__numberColumn {
		min-width: 35px;
		color: var(--clr-text-3);
		border-color: var(--clr-border-2);
		background-color: var(--clr-bg-1-muted);
		font-size: 11px;
		border-right-width: 1px;
		vertical-align: top;
		padding-left: 4px;
		padding-right: 4px;
		text-align: right;
		cursor: var(--cursor);
		user-select: none;
	}

	tr:first-of-type .table__numberColumn:first-child {
		border-radius: var(--radius-s) 0 0 0;
	}
	tr:last-of-type .table__numberColumn:first-child {
		border-radius: 0 0 0 var(--radius-s);
	}

	.diff-line-deletion {
		background-color: #cf8d8e20;
	}

	.diff-line-addition {
		background-color: #94cf8d20;
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
