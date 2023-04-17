<script lang="ts">
	import { create } from '$lib/components/CodeViewer/CodeHighlighter';

	export let diff: string;
	export let path: string;

	const sanitize = (text: string) => {
		var element = document.createElement('div');
		element.innerText = text;
		return element.innerHTML;
	};

	let currentDiff = '';
	let middleDiff = '';
	let currentOffset = 0;

	let htmlTagRegex = /(<([^>]+)>)/gi;

	$: if (diff) {
		middleDiff = '';
		currentDiff = '';
		currentOffset = 0;
		let lineClass = '';

		let doc = create(diff, path);
		doc.highlightRange(0, diff.length, (text, style) => {
			middleDiff += style ? `<span class=${style}>${sanitize(text)}</span>` : sanitize(text);
		});

		let diffLines = middleDiff.split('<br>');
		diffLines.forEach((line, index) => {
			lineClass = 'lineContext ';
			let firstChar = line.replace(htmlTagRegex, '').slice(0, 1);
			if (index < 4) {
				lineClass = 'lineDiff bg-zinc-900 text-zinc-500';
			} else if (line.slice(0, 2) == '@@') {
				lineClass = 'lineSplit bg-blue-900';
			} else if (firstChar == '+') {
				if (!line.includes('+++')) {
					lineClass = 'lineSplit bg-green-900';
				}
			} else if (firstChar == '-') {
				if (!line.includes('---')) {
					lineClass = 'lineSplit bg-red-900';
				}
			}
			currentDiff += `<div class="${lineClass}">`;
			currentDiff += line;
			currentDiff += '</div>';
			currentOffset += line.length;
		});
	}
</script>

<pre class="h-full w-full">{@html currentDiff}</pre>
