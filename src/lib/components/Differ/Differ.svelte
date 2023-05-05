<script lang="ts">
	import { create } from './CodeHighlighter';
	import { buildDiffRows, documentMap, RowType, type Row } from './renderer';

	import './diff.css';
	import './colors/gruvbox-alternative.css';
	import type { DiffArray } from './diff';

	export let filepath: string;
	export let highlight: string[] = [];
	export let paddingLines = 10000;
	export let diff: DiffArray;

	const sanitize = (text: string) => {
		var element = document.createElement('div');
		element.innerText = text;
		return element.innerHTML;
	};

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

	$: renderedRows = diffRows.rows.map((row) => ({ ...row, render: renderRowContent(row) }));

	type RenderedRow = (typeof renderedRows)[0];

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

	$: rows = highlight.length > 0 ? padHighlighted(renderedRows) : renderedRows;

	const scrollToChangedLine = () => {
		const changedLines = document.getElementsByClassName('line-changed');
		if (changedLines.length > 0) {
			changedLines[0].scrollIntoView({ behavior: 'smooth', block: 'center' });
		}
	};
	$: diff && scrollToChangedLine();
</script>

<div
	id="content"
	class="grid h-full w-full flex-auto select-text whitespace-pre border-b border-zinc-800 font-mono"
	style:grid-template-columns="minmax(auto, max-content) minmax(auto, max-content) 1fr"
>
	{#each rows as row}
		{@const baseNumber =
			row.type === RowType.Equal || row.type === RowType.Deletion
				? String(row.originalLineNumber)
				: ''}
		{@const curNumber =
			row.type === RowType.Equal || row.type === RowType.Addition
				? String(row.currentLineNumber)
				: ''}
		<span class="select-none border-r  border-zinc-800 bg-zinc-900 text-zinc-500/70">
			<div class="mx-1.5 text-right">
				{baseNumber}
			</div>
		</span>

		<span class="mr-1 select-none border-r border-zinc-800 bg-zinc-900 text-zinc-500/70">
			<div class="mx-1.5 text-right">
				{curNumber}
			</div>
		</span>

		<span
			class="diff-line-{row.type} overflow-hidden whitespace-pre-wrap"
			class:line-changed={row.type === RowType.Addition || row.type === RowType.Deletion}
		>
			{#each row.render.html as content}
				{@html content}
			{/each}
		</span>
	{/each}
</div>
