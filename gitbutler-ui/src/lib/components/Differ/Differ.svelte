<script lang="ts">
	import { create } from './CodeHighlighter';
	import { buildDiffRows, documentMap, RowType, type Row } from './renderer';
	import type { DiffArray } from '$lib/diff';

	export let filepath: string;
	export let highlight: string[] = [];
	export let paddingLines = 10000;
	export let diff: DiffArray;

	function sanitize(text: string) {
		var element = document.createElement('div');
		element.innerText = text;
		return element.innerHTML;
	}

	$: diffRows = buildDiffRows(diff, { paddingLines });
	$: originalHighlighter = create(diffRows.originalLines.join('\n'), filepath);
	$: originalMap = documentMap(diffRows.originalLines);
	$: currentHighlighter = create(diffRows.currentLines.join('\n'), filepath);
	$: currentMap = documentMap(diffRows.currentLines);

	function markRanges(row: Row, highlight: string[]): [number, number][] {
		if (row.type !== RowType.Addition && row.type !== RowType.Deletion) return [];
		let ranges: [number, number][] = [];
		const line = row.tokens.reduce((acc, token) => acc + token.text, '');
		for (const h of highlight) {
			let pos = 0;
			let index = line.indexOf(h, pos);
			while (index !== -1) {
				ranges.push([index, index + h.length]);
				pos = index + h.length;
				index = line.indexOf(h, pos);
			}
		}
		ranges = mergeTouchingRanges(ranges);
		return ranges;
	}

	function mergeTouchingRanges(ranges: [number, number][]): [number, number][] {
		ranges = ranges.sort((a, b) => a[0] - b[0]);
		const merged: [number, number][][] = [];
		for (const range of ranges) {
			const touching = merged.find((r) => r.some((t) => t[1] === range[0] || t[0] === range[1]));
			if (touching) {
				touching.push(range);
			} else {
				merged.push([range]);
			}
		}
		return merged.map((r) => [Math.min(...r.map((t) => t[0])), Math.max(...r.map((t) => t[1]))]);
	}

	function isIntersecting(a: [number, number], b: [number, number]): boolean {
		if (a[0] > b[0]) return isIntersecting(b, a);
		if (a[1] <= b[0]) return false;
		if (a[0] <= b[0] && b[0] <= a[1]) return true;
		if (b[0] <= a[0] && b[0] <= b[1]) return true;
		return false;
	}

	function renderRowContent(row: Row): { html: string[]; highlighted: boolean } {
		if (row.type === RowType.Spacer) {
			return { html: row.tokens.map((tok) => `${tok.text}`), highlighted: false };
		}

		const [doc, startPos] =
			row.type === RowType.Deletion
				? [originalHighlighter, originalMap.get(row.originalLineNumber) as number]
				: [currentHighlighter, currentMap.get(row.currentLineNumber) as number];

		const content: string[] = [];
		let pos = startPos;

		const mark = markRanges(row, highlight).map(([start, end]) => [start + pos, end + pos]);

		let highlighted = mark.length > 0;
		for (const token of row.tokens) {
			let tokenContent = '';

			let tokenPos = pos;
			doc.highlightRange(pos, pos + token.text.length, (text, classNames) => {
				const token = classNames
					? `<span class=${classNames}>${sanitize(text)}</span>`
					: sanitize(text);

				const isHighlighted = mark.some(([from, to]) => {
					const is = isIntersecting([from, to], [tokenPos, tokenPos + text.length]);
					return is;
				});
				tokenPos += text.length;

				tokenContent += isHighlighted ? `<mark>${token}</mark>` : token;
			});

			content.push(
				token.className
					? `<span class=${token.className}>${tokenContent}</span>`
					: `${tokenContent}`
			);

			pos += token.text.length;
		}

		return { html: content, highlighted };
	}

	$: renderedRows = diffRows.rows.map((row) => ({ ...row, render: renderRowContent(row) }));

	type RenderedRow = (typeof renderedRows)[0];

	function padHighlighted(rows: RenderedRow[]): RenderedRow[] {
		const chunks: (RenderedRow[] | RenderedRow)[] = [];

		function mergeChunk(rows: RenderedRow[], isFirst: boolean, isLast: boolean): RenderedRow[] {
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
		}

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
	}

	$: rows = highlight.length > 0 ? padHighlighted(renderedRows) : renderedRows;

	function scrollToChangedLine() {
		const changedLines = document.getElementsByClassName('line-changed');
		if (changedLines.length > 0) {
			changedLines[0].scrollIntoView({ behavior: 'smooth', block: 'center' });
		}
	}
	$: diff && scrollToChangedLine();
</script>

<div
	id="content"
	class="border-color-4 grid h-full w-full flex-auto select-text whitespace-pre border-t font-mono"
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
		<span class="bg-color-3 border-color-4 text-color-1 select-none border-l border-r">
			<div class="mx-1.5 text-right">
				{baseNumber}
			</div>
		</span>

		<span class="bg-color-3 border-color-4 text-color-1 select-none border-r">
			<div class="mx-1.5 text-right">
				{curNumber}
			</div>
		</span>

		<span
			class="diff-line-{row.type} bg-color-5 cursor-text overflow-hidden whitespace-pre-wrap"
			class:line-changed={row.type === RowType.Addition || row.type === RowType.Deletion}
		>
			{#each row.render.html as content}
				{@html content}
			{/each}
		</span>
	{/each}
</div>
