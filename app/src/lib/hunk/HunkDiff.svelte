<script lang="ts">
	import { create } from '$lib/utils/codeHighlight';
	import { type ContentSection, SectionType, type Line } from '$lib/utils/fileSections';
	import diff_match_patch from 'diff-match-patch';
	import type { Hunk } from '$lib/vbranches/types';

	interface Props {
		hunk: Hunk;
		filePath: string;
		subsections: ContentSection[];
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

	const { hunk, filePath, subsections }: Props = $props();

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

	function calculateWordDiff(prevSection: ContentSection, nextSection: ContentSection): DiffRows {
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

	function emptyLines(lines: Line[]) {
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
			// If there's no prevLine (first line) or the target line is of type Context (not add or rm), skip
			if (!prevSection || nextSection.sectionType === SectionType.Context) {
				acc.push(...createRowData(nextSection));
				return acc;
			}

			// Skip sections which aren't the same length in lines changed
			if (prevSection.lines.length !== nextSection.lines.length) {
				acc.push(...createRowData(nextSection));
				return acc;
			}

			// Skip sections where previous line is empty
			if (emptyLines(prevSection.lines)) return acc;

			// Calculate word diffs on all remaining sections
			const { prevRows, nextRows } = calculateWordDiff(prevSection, nextSection);

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
</script>

<table data-hunk-hash={hunk.hash} class="table__section">
	<tbody>
		<tr>
			<td colspan="3">{filePath}</td>
		</tr>
		{#each renderRows as line}
			<tr>
				<td class="table__numberColumn" align="center">{line.originalLineNumber}</td>
				<td class="table__numberColumn" align="center">{line.currentLineNumber}</td>
				<td
					class="table__textContent"
					class:diff-line-deletion={line.type === SectionType.RemovedLines}
					class:diff-line-addition={line.type === SectionType.AddedLines}
				>
					<span class="blob-code-content">
						{@html line.tokens.join('')}
					</span>
				</td>
			</tr>
		{/each}
	</tbody>
</table>

<style>
	.table__section {
		width: 100%;
	}

	.table__numberColumn {
		padding-inline: 0.35rem;
	}

	.diff-line-deletion {
		background-color: #cf8d8e20;
	}

	.diff-line-addition {
		background-color: #94cf8d20;
	}

	.blob-code-content {
		font-family: 'monospace';
		white-space: pre;
		user-select: text;

		&:hover {
			cursor: text;
		}
	}
</style>
