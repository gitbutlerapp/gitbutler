<script lang="ts">
	import { SectionType } from './fileSections';
	import type { Line } from './fileSections';
	import { create } from '$lib/components/Differ/CodeHighlighter';
	import { createEventDispatcher } from 'svelte';

	export let line: Line;
	export let sectionType: SectionType;
	export let filePath: string;
	export let minWidth = 1.75;
	export let selectable: boolean = false;
	export let selected: boolean = true;

	const dispatch = createEventDispatcher<{ selected: boolean }>();

	function toTokens(codeString: string): string[] {
		function sanitize(text: string) {
			var element = document.createElement('div');
			element.innerText = text.replace('   ', ' ').replace(/\t/g, ' ');
			return element.innerHTML;
		}

		let highlighter = create(codeString, filePath);
		let tokens: string[] = [];
		highlighter.highlight((text, classNames) => {
			const token = classNames
				? `<span class=${classNames}>${sanitize(text)}</span>`
				: sanitize(text);

			tokens.push(token);
		});
		return tokens;
	}

	$: bgColor =
		selectable && selected
			? 'bg-blue-400 border-blue-500 text-white dark:border-blue-700 dark:bg-blue-800'
			: 'bg-light-50 border-light-300 dark:bg-dark-700 dark:border-dark-400';
</script>

<div
	class="flex w-full bg-light-50 font-mono text-sm dark:bg-dark-700"
	role="group"
	on:contextmenu|preventDefault
>
	<button
		disabled={!selectable}
		on:click={() => dispatch('selected', !selected)}
		class="shrink-0 select-none border-r px-0.5 text-right text-xs text-light-600 dark:text-light-300 {bgColor}"
		style:min-width={minWidth + 'rem'}
	>
		{line.beforeLineNumber || ''}
	</button>
	<button
		disabled={!selectable}
		on:click={() => dispatch('selected', !selected)}
		class="text-color-4 shrink-0 select-none border-r border-light-400 px-0.5 text-right text-xs dark:border-dark-400 {bgColor}"
		style:min-width={minWidth + 'rem'}
	>
		{line.afterLineNumber || ''}
	</button>
	<div
		class="flex-grow overflow-hidden whitespace-pre pl-0.5"
		class:diff-line-deletion={sectionType === SectionType.RemovedLines}
		class:diff-line-addition={sectionType === SectionType.AddedLines}
	>
		{@html toTokens(line.content).join('')}
	</div>
</div>
