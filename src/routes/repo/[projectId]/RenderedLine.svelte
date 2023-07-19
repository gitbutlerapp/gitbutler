<script lang="ts">
	import { SectionType } from './fileSections';
	import type { Line } from './fileSections';
	import { create } from '$lib/components/Differ/CodeHighlighter';

	export let line: Line;
	export let sectionType: SectionType;
	export let filePath: string;
	export let minWidth = 1.75;
	export let maximized: boolean;

	function toTokens(codeString: string): string[] {
		function sanitize(text: string) {
			var element = document.createElement('div');
			element.innerText = text;
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
</script>

<div class="flex w-full font-mono text-sm" role="group" on:contextmenu|preventDefault>
	<div
		class="shrink-0 select-none border-r border-light-400 bg-light-50 px-1 text-right text-light-600 dark:border-dark-400 dark:bg-dark-800 dark:text-light-300"
		style:min-width={minWidth + 'rem'}
	>
		{line.beforeLineNumber || ''}
	</div>
	<div
		class="shrink-0 select-none border-r border-light-400 bg-light-50 px-1 text-right text-light-600 dark:border-dark-400 dark:bg-dark-800 dark:text-light-300"
		style:min-width={minWidth + 'rem'}
	>
		{line.afterLineNumber || ''}
	</div>
	<div
		class="flex-grow overflow-hidden"
		class:whitespace-pre={maximized}
		class:whitespace-nowrap={!maximized}
		class:diff-line-deletion={sectionType === SectionType.RemovedLines}
		class:diff-line-addition={sectionType === SectionType.AddedLines}
	>
		{@html toTokens(line.content).join('')}
	</div>
</div>
