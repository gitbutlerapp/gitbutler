<script lang="ts">
	import { create } from '$lib/components/Differ/CodeHighlighter';
	import { SectionType } from '$lib/utils/fileSections';
	import { createEventDispatcher } from 'svelte';
	import type { Line } from '$lib/utils/fileSections';

	export let line: Line;
	export let sectionType: SectionType;
	export let filePath: string;
	export let minWidth = 1.75;
	export let selectable: boolean = false;
	export let selected: boolean = true;
	export let readonly: boolean = false;
	export let draggingDisabled: boolean = false;
	export let tabSize = 4;

	const dispatch = createEventDispatcher<{ selected: boolean }>();

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

	$: isSelected = selectable && selected;
</script>

<div class="code-line" role="group" style="--tab-size: {tabSize}" on:contextmenu|preventDefault>
	<div class="code-line__numbers-line">
		<button
			on:click={() => selectable && dispatch('selected', !selected)}
			class="numbers-line-count"
			class:selected={isSelected}
			style:min-width={minWidth + 'rem'}
			style:cursor={draggingDisabled ? 'default' : 'grab'}
		>
			{line.beforeLineNumber || ''}
		</button>
		<button
			on:click={() => selectable && dispatch('selected', !selected)}
			class="numbers-line-count"
			class:selected={isSelected}
			style:min-width={minWidth + 'rem'}
			style:cursor={draggingDisabled ? 'default' : 'grab'}
		>
			{line.afterLineNumber || ''}
		</button>
	</div>
	<div
		class="line"
		class:readonly
		class:diff-line-deletion={sectionType === SectionType.RemovedLines}
		class:diff-line-addition={sectionType === SectionType.AddedLines}
		style:cursor={draggingDisabled ? 'default' : 'grab'}
	>
		<span class="selectable-wrapper" data-no-drag>
			{@html toTokens(line.content).join('')}
		</span>
	</div>
</div>

<style lang="postcss">
	.code-line {
		display: flex;
		width: 100%;
		min-width: max-content;
		font-family: monospace;
		background-color: var(--clr-bg-1);
		white-space: pre;
		tab-size: var(--tab-size);

		font-size: 0.688rem;
		line-height: 1.5;
	}

	.line {
		flex-grow: 1;
	}

	.code-line__numbers-line {
		position: sticky;
		left: 0;
		display: flex;
	}

	.numbers-line-count {
		color: var(--clr-text-3);
		border-color: var(--clr-border-2);
		background-color: var(--clr-bg-1-muted);
		font-size: 0.625rem;
		flex-shrink: 0;
		user-select: none;
		border-right-width: 1px;
		padding-left: 0.125rem;
		padding-right: 0.125rem;
		text-align: right;

		&.selected {
			background-color: #60a5fa;
			border-color: #2563eb;
			color: white;
		}
	}

	.selectable-wrapper {
		cursor: text;
		display: inline-block;
		text-indent: var(--size-4);
		margin-right: var(--size-4);
	}
</style>
