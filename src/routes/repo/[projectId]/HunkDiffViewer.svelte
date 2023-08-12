<script lang="ts">
	import { buildDiffRows, documentMap, RowType, type Row } from '$lib/components/Differ/renderer';
	import { line, type DiffArray } from '$lib/diff';
	import { create } from '$lib/components/Differ/CodeHighlighter';

	export let diff: string;
	export let filePath: string;
	export let linesShown: number;

	function parseHunk(
		diff: string,
		lineLimit: number
	): {
		diffRows: ReturnType<typeof buildDiffRows>;
		originalLineNumber: number;
		currentLineNumber: number;
	} {
		const diffLines = diff.split('\n');
		const header = diffLines[0];

		const lr = header.split('@@')[1].trim().split(' ');
		const originalLineNumber = parseInt(lr[0].split(',')[0].slice(1));
		const currentLineNumber = parseInt(lr[1].split(',')[0].slice(1));

		const before = diffLines
			.filter((line) => line.startsWith('-'))
			.map((line) => line.slice(1))
			.slice(0, lineLimit / 2);
		const after = diffLines
			.filter((line) => line.startsWith('+'))
			.map((line) => line.slice(1))
			.slice(0, lineLimit / 2);

		const diffArray: DiffArray = line(before, after);
		const diffRows = buildDiffRows(diffArray, { paddingLines: 10000 });

		return { diffRows, originalLineNumber, currentLineNumber };
	}

	function renderRowContent(row: Row): { html: string[] } {
		if (row.type === RowType.Spacer) {
			return { html: row.tokens.map((tok) => `${tok.text}`) };
		}

		const [doc, startPos] =
			row.type === RowType.Deletion
				? [originalHighlighter, originalMap.get(row.originalLineNumber) as number]
				: [currentHighlighter, currentMap.get(row.currentLineNumber) as number];

		const content: string[] = [];
		let pos = startPos;

		for (const token of row.tokens) {
			let tokenContent = '';

			doc.highlightRange(pos, pos + token.text.length, (text, classNames) => {
				const token = classNames
					? `<span class=${classNames}>${sanitize(text)}</span>`
					: sanitize(text);

				tokenContent += token;
			});

			content.push(
				token.className
					? `<span class=${token.className}>${tokenContent}</span>`
					: `${tokenContent}`
			);

			pos += token.text.length;
		}

		return { html: content };
	}

	function sanitize(text: string) {
		var element = document.createElement('div');
		element.innerText = text;
		return element.innerHTML;
	}

	$: parsedHunk = parseHunk(diff, linesShown);
	$: diffRows = parsedHunk.diffRows;
	$: originalLineNumber = parsedHunk.originalLineNumber;
	$: currentLineNumber = parsedHunk.currentLineNumber;
	$: originalHighlighter = create(diffRows.originalLines.join('\n'), filePath);
	$: originalMap = documentMap(diffRows.originalLines);
	$: currentHighlighter = create(diffRows.currentLines.join('\n'), filePath);
	$: currentMap = documentMap(diffRows.currentLines);
	$: renderedRows = diffRows.rows.map((row) => ({ ...row, render: renderRowContent(row) }));
</script>

<div
	class="grid h-full w-full flex-auto whitespace-pre font-mono text-sm"
	style:grid-template-columns="minmax(auto, max-content) minmax(auto, max-content) 1fr"
>
	{#each renderedRows as row}
		{@const baseNumber =
			row.type === RowType.Equal || row.type === RowType.Deletion
				? String(row.originalLineNumber + originalLineNumber - 1)
				: ''}
		{@const curNumber =
			row.type === RowType.Equal || row.type === RowType.Addition
				? String(row.currentLineNumber + currentLineNumber - 1)
				: ''}
		<span
			class="min-w-[1rem] select-none border-r border-light-400 bg-light-200 px-1 text-right text-light-800 dark:border-dark-400 dark:bg-dark-800 dark:text-light-300"
		>
			{baseNumber}
		</span>

		<span
			class="min-w-[1rem] select-none border-r border-light-400 bg-light-200 px-1 text-right text-light-800 dark:border-dark-400 dark:bg-dark-800 dark:text-light-300"
		>
			{curNumber}
		</span>
		<span
			class="pl-1 diff-line-{row.type} overflow-hidden whitespace-pre"
			class:line-changed={row.type === RowType.Addition || row.type === RowType.Deletion}
		>
			{#each row.render.html as content}
				{@html content}
			{/each}
		</span>
	{/each}
</div>

<style lang="postcss">
</style>
