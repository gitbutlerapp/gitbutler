<script lang="ts">
	import { type Delta, Operation } from '$lib/deltas';
	import { lineDiff } from './diff';
	import { create } from './CodeHighlighter';
	import { buildDiffRows, documentMap, RowType, type Row } from './renderer';

	import './diff.css';
	import './colors/gruvbox.css';

	export let doc: string;
	export let deltas: Delta[];
	export let filepath: string;
	export let context: number = 1000;

	const applyDeltas = (text: string, deltas: Delta[]) => {
		const operations = deltas.flatMap((delta) => delta.operations);

		operations.forEach((operation) => {
			if (Operation.isInsert(operation)) {
				text =
					text.slice(0, operation.insert[0]) +
					operation.insert[1] +
					text.slice(operation.insert[0]);
			} else if (Operation.isDelete(operation)) {
				text =
					text.slice(0, operation.delete[0]) +
					text.slice(operation.delete[0] + operation.delete[1]);
			}
		});
		return text;
	};

	$: left = deltas.length > 0 ? applyDeltas(doc, deltas.slice(0, deltas.length - 1)) : doc;
	$: right = deltas.length > 0 ? applyDeltas(left, deltas.slice(deltas.length - 1)) : left;
	$: diff = lineDiff(left.split('\n'), right.split('\n'));

	$: diffRows = buildDiffRows(diff, context);
	$: originalHighlighter = create(diffRows.originalLines.join('\n'), filepath);
	$: originalMap = documentMap(diffRows.originalLines);
	$: currentHighlighter = create(diffRows.currentLines.join('\n'), filepath);
	$: currentMap = documentMap(diffRows.currentLines);

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

<div class="diff-listing w-full select-text whitespace-pre font-mono">
	{#each diffRows.rows as row}
		{@const baseNumber =
			row.type === RowType.Equal || row.type === RowType.Deletion
				? String(row.originalLineNumber)
				: ''}
		{@const curNumber =
			row.type === RowType.Equal || row.type === RowType.Addition
				? String(row.currentLineNumber)
				: ''}
		<div class="select-none pr-1 pl-2.5 text-right text-[#665c54]">{baseNumber}</div>
		<div class="select-none pr-1 pl-2.5 text-right text-[#665c54]">{curNumber}</div>
		<div
			class="diff-line-marker"
			class:diff-line-addition={row.type === RowType.Addition}
			class:diff-line-deletion={row.type === RowType.Deletion}
		>
			{row.type === RowType.Addition ? '+' : row.type === RowType.Deletion ? '-' : ''}
		</div>
		<div
			class:line-changed={row.type === RowType.Addition || row.type === RowType.Deletion}
			class="px-1 diff-line-{row.type}"
			data-line-number={curNumber}
		>
			{#each renderRowContent(row) as content}
				{@html content}
			{/each}
		</div>
	{/each}
</div>
