<script lang="ts">
	import { create } from '$lib/utils/codeHighlight';
	import { type ContentSection, SectionType, type Line } from '$lib/utils/fileSections';
	import diff_match_patch from 'diff-match-patch';
	import type { Hunk } from '$lib/vbranches/types';

	type HandleLineContextMenuArgs = {
		event: MouseEvent;
		lineNumber: number;
		hunk: Hunk;
		subsection: ContentSection;
	};

	interface Props {
		hunk: Hunk;
		filePath: string;
		selectable: boolean;
		subsections: ContentSection[];
		handleSelected?: (hunk: Hunk, isSelected: boolean) => void;
		handleClick?: () => void;
		handleLineContextMenu?: ({
			event,
			lineNumber,
			hunk,
			subsection
		}: HandleLineContextMenuArgs) => void;
	}

	interface Row {
		originalLineNumber?: number;
		currentLineNumber?: number;
		tokens: string[];
		type: SectionType;
		size: number;
	}

	enum Operation {
		Equal = 0,
		Insert = 1,
		Delete = -1,
		Edit = 2
	}

	type DiffRows = { prevRows: Row[]; nextRows: Row[] };

	const {
		hunk,
		filePath,
		selectable,
		subsections,
		handleSelected,
		handleClick,
		handleLineContextMenu
	}: Props = $props();

	function charDiff(text1: string, text2: string): { 0: number; 1: string }[] {
		const differ = new diff_match_patch();
		const diff = differ.diff_main(text1, text2);
		return diff;
	}

	function sanitize(text: string) {
		var element = document.createElement('div');
		element.innerText = text;
		return element.innerHTML;
	}

	function createRowData(section: ContentSection): Row[] {
		return section.lines.map((line) => {
			return {
				originalLineNumber: line.beforeLineNumber,
				currentLineNumber: line.afterLineNumber,
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
				? `<span class=${classNames}>${sanitize(text)}</span>`
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
				originalLineNumber: oldLine.beforeLineNumber,
				currentLineNumber: oldLine.afterLineNumber,
				tokens: [] as string[],
				type: prevSection.sectionType,
				size: oldLine.content.length
			};
			const nextSectionRow = {
				originalLineNumber: newLine.beforeLineNumber,
				currentLineNumber: newLine.afterLineNumber,
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

	function isLineEmpty(lines: Line[]) {
		const whitespaceRegex = new RegExp(/\s/);
		if (!lines[0].content.match(whitespaceRegex)) {
			return true;
		}

		return false;
	}

	// Filter out section for which we don't need to compute word diffs
	function filterRows(subsections: ContentSection[]) {
		return subsections.reduce((acc, nextSection, i) => {
			const prevSection = subsections[i - 1];
			if (!prevSection || nextSection.sectionType === SectionType.Context) {
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
			// 1. Find and replace previous rows with tokenized version
			prevRows.forEach((row) => {
				const accIndex = acc.findIndex(
					(accRow) =>
						accRow.originalLineNumber === row.originalLineNumber &&
						accRow.currentLineNumber === row.currentLineNumber
				);
				if (!accIndex) return;

				acc[accIndex] = row;
			});

			// 2. Push Tokenized nextRows onto end of array
			acc.push(...nextRows);

			return acc;
		}, [] as Row[]);
	}

	const renderRows = $derived(filterRows(subsections));

	const selected = true;
	let isSelected = $derived(selectable && selected);

	$inspect('renderRows', renderRows);
</script>

<div class="table__wrapper">
	<table data-hunk-id={hunk.id} class="table__section">
		<tbody>
			{#each renderRows as line}
				<tr>
					<td
						class="table__numberColumn"
						align="center"
						class:selected={isSelected}
						onclick={() => selectable && handleSelected?.(hunk, true)}
					>
						{line.originalLineNumber}
					</td>
					<td
						class="table__numberColumn"
						align="center"
						class:selected={isSelected}
						onclick={(event) => {
							selectable && handleSelected?.(hunk, true);
							const lineNumber = (line.originalLineNumber
								? line.originalLineNumber
								: line.currentLineNumber) as number;
							handleLineContextMenu?.({ event, hunk, lineNumber, subsection: subsections[0] });
						}}
					>
						{line.currentLineNumber}
					</td>
					<td
						class="table__textContent"
						class:diff-line-deletion={line.type === SectionType.RemovedLines}
						class:diff-line-addition={line.type === SectionType.AddedLines}
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
		border-radius: var(--radius-m);
	}
	.table__section {
		width: 100%;
		font-family: 'monospace';
	}

	.table__numberColumn {
		width: 1%;
		padding-inline: 0.35rem;
		color: var(--clr-text-3);
		border-color: var(--clr-border-2);
		background-color: var(--clr-bg-1-muted);
		font-size: 10px;
		border-right-width: 1px;
		padding-left: 2px;
		padding-right: 2px;
		text-align: right;
		min-width: var(--minwidth);
		cursor: var(--cursor);
	}

	tr:first-of-type .table__numberColumn:first-child {
		border-radius: var(--radius-m) 0 0 0;
	}
	tr:last-of-type .table__numberColumn:first-child {
		border-radius: 0 0 0 var(--radius-m);
	}

	.diff-line-deletion {
		background-color: #cf8d8e20;
	}

	.diff-line-addition {
		background-color: #94cf8d20;
	}

	.table__textContent {
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
