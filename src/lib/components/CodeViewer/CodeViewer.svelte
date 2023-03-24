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
	export let highlight: string[] = [];
	export let paddingLines = 10000;

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

	const sanitize = (text: string) => {
		var element = document.createElement('div');
		element.innerText = text;
		return element.innerHTML;
	};

	$: left = deltas.length > 0 ? applyDeltas(doc, deltas.slice(0, deltas.length - 1)) : doc;
	$: right = deltas.length > 0 ? applyDeltas(left, deltas.slice(deltas.length - 1)) : left;
	$: diff = lineDiff(left.split('\n'), right.split('\n'));

	$: diffRows = buildDiffRows(diff, { paddingLines });
	$: originalHighlighter = create(diffRows.originalLines.join('\n'), filepath);
	$: originalMap = documentMap(diffRows.originalLines);
	$: currentHighlighter = create(diffRows.currentLines.join('\n'), filepath);
	$: currentMap = documentMap(diffRows.currentLines);

	const renderRowContent = (row: Row): { html: string[]; highlighted: boolean } => {
		if (row.type === RowType.Spacer) {
			return { html: row.tokens.map((tok) => `${tok.text}`), highlighted: false };
		}

		const [doc, startPos] =
			row.type === RowType.Deletion
				? [originalHighlighter, originalMap.get(row.originalLineNumber) as number]
				: [currentHighlighter, currentMap.get(row.currentLineNumber) as number];

		const content: string[] = [];
		let pos = startPos;

		let highlighted = false;
		for (const token of row.tokens) {
			let tokenContent = '';

			doc.highlightRange(pos, pos + token.text.length, (text, classNames) => {
				const token = classNames
					? `<span class=${classNames}>${sanitize(text)}</span>`
					: sanitize(text);

				const shouldHighlight =
					(row.type === RowType.Deletion || row.type === RowType.Addition) &&
					highlight.find((h) => text.includes(h));

				if (shouldHighlight) highlighted = true;

				tokenContent += shouldHighlight ? `<mark>${token}</mark>` : token;
			});

			content.push(
				token.className
					? `<span class=${token.className}>${tokenContent}</span>`
					: `${tokenContent}`
			);

			pos += token.text.length;
		}

		return { html: content, highlighted };
	};

	$: rows = diffRows.rows.map((row) => ({ ...row, render: renderRowContent(row) }));

	type RenderedRow = (typeof rows)[0];

	const padHighlighted = (rows: RenderedRow[]): RenderedRow[] => {
		const chunks: (RenderedRow[] | RenderedRow)[] = [];

		const mergeChunk = (rows: RenderedRow[], isFirst: boolean, isLast: boolean): RenderedRow[] => {
			const spacerIndex = rows.findIndex((row) => row.type === RowType.Spacer);
			if (spacerIndex === -1) {
				if (isFirst) {
					return rows.slice(-paddingLines);
				} else if (isLast) {
					return rows.slice(0, paddingLines);
				} else {
					return [
						...rows.slice(0, paddingLines),
						{
							originalLineNumber: -1,
							currentLineNumber: -1,
							type: RowType.Spacer,
							tokens: [{ text: '...' }],
							render: { html: ['...'], highlighted: false }
						},
						...rows.slice(-paddingLines)
					] as RenderedRow[];
				}
			} else {
				let beforeSpacer = rows.slice(0, spacerIndex);
				let afterSpacer = rows.slice(spacerIndex + 1);
				if (isFirst) {
					return afterSpacer.slice(-paddingLines);
				} else if (isLast) {
					return beforeSpacer.slice(0, paddingLines);
				} else {
					return [
						...beforeSpacer.slice(0, paddingLines),
						{
							originalLineNumber: -1,
							currentLineNumber: -1,
							type: RowType.Spacer,
							tokens: [{ text: '...' }],
							render: { html: ['...'], highlighted: false }
						},
						...afterSpacer.slice(-paddingLines)
					] as RenderedRow[];
				}
			}
		};

		for (const row of rows) {
			if (row.render.highlighted) {
				if (chunks.length > 0) {
					const lastChunk = chunks[chunks.length - 1];
					if (Array.isArray(lastChunk)) {
						chunks[chunks.length - 1] = mergeChunk(lastChunk, chunks.length === 1, false);
					}
				}
				chunks.push(row);
			} else {
				if (chunks.length === 0) {
					chunks.push([row]);
				} else {
					const lastChunk = chunks[chunks.length - 1];
					if (Array.isArray(lastChunk)) {
						lastChunk.push(row);
					} else {
						chunks.push([row]);
					}
				}
			}
		}
		const lastChunk = chunks[chunks.length - 1];
		if (Array.isArray(lastChunk)) {
			chunks[chunks.length - 1] = mergeChunk(lastChunk, false, true);
		}
		return chunks.flatMap((chunk) => chunk);
	};
</script>

<div class="diff-listing w-full select-text whitespace-pre font-mono">
	{#each highlight.length > 0 ? padHighlighted(rows) : rows as row}
		{@const baseNumber =
			row.type === RowType.Equal || row.type === RowType.Deletion
				? String(row.originalLineNumber)
				: ''}
		{@const curNumber =
			row.type === RowType.Equal || row.type === RowType.Addition
				? String(row.currentLineNumber)
				: ''}
		<div class="select-none pr-1 pl-2.5 text-right text-[#8C8178]">{baseNumber}</div>
		<div class="select-none pr-1 pl-2.5 text-right text-[#8C8178]">{curNumber}</div>
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
			{#each row.render.html as content}
				{@html content}
			{/each}
		</div>
	{/each}
</div>
