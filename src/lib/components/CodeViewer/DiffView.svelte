<script lang="ts">
	import type { DiffArray } from './diff';
	import { create } from './CodeHighlighter';
	import { buildDiffRows, documentMap, RowType, type Row } from './renderer';
	import './diff.css';
	import './highlight.css';

	export let diff: DiffArray;
	export let filepath: string;

	$: diffRows = buildDiffRows(diff);
	$: originalHighlighter = create(diffRows.originalLines.join('\n'), filepath);
	$: originalMap = documentMap(diffRows.originalLines);
	$: currentHighlighter = create(diffRows.currentLines.join('\n'), filepath);
	$: currentMap = documentMap(diffRows.currentLines);

	const rowAttrs = (row: Row) => {
		const baseNumber =
			row.type === RowType.Equal || row.type === RowType.Deletion
				? String(row.originalLineNumber)
				: '';
		const curNumber =
			row.type === RowType.Equal || row.type === RowType.Addition
				? String(row.currentLineNumber)
				: '';
		let marker = '',
			markerClass = 'diff-line-marker';
		if (row.type === RowType.Addition) {
			marker = '+';
			markerClass += ' diff-line-addition';
		} else if (row.type === RowType.Deletion) {
			marker = '-';
			markerClass += ' diff-line-deletion';
		}

		return { baseNumber, curNumber, marker, markerClass };
	};

	const renderRowContent = (row: Row) => {
		if (row.type === RowType.Spacer) {
			return row.tokens.map((tok) => `${tok.text}`);
		}
		const [doc, startPos] =
			row.type === RowType.Deletion
				? [originalHighlighter, originalMap.get(row.originalLineNumber) as number]
				: [currentHighlighter, currentMap.get(row.currentLineNumber) as number];
		const content: string[] = [];
		let pos = startPos;

		const sanitize = (text: string) => {
			var element = document.createElement('div');
			element.innerText = text;
			return element.innerHTML;
		};

		for (const token of row.tokens) {
			let tokenContent = '';
			doc.highlightRange(pos, pos + token.text.length, (text, style) => {
				tokenContent += style ? `<span class=${style}>${sanitize(text)}</span>` : sanitize(text);
			});

			content.push(
				token.className
					? `<span class=${token.className}>${tokenContent}</span>`
					: `${tokenContent}`
			);
			pos += token.text.length;
		}
		return content;
	};
</script>

<div class="diff-listing h-full w-full">
	{#each diffRows.rows as row}
		{@const { baseNumber, curNumber, marker, markerClass } = rowAttrs(row)}
		<div class="diff-line-number">{baseNumber}</div>
		<div class="diff-line-number">{curNumber}</div>
		<div class={markerClass}>{marker}</div>
		<div class="diff-line-content diff-line-{row.type}" data-line-number={curNumber}>
			{#each renderRowContent(row) as content}
				{@html content}
			{/each}
		</div>
	{/each}
</div>
