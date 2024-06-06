<script lang="ts">
	import HunkContextMenu from './HunkContextMenu.svelte';
	import { Project } from '$lib/backend/projects';
	import { create } from '$lib/components/Differ/CodeHighlighter';
	import { getContext } from '$lib/utils/context';
	import { SectionType } from '$lib/utils/fileSections';
	import { onDestroy } from 'svelte';
	import { createEventDispatcher } from 'svelte';
	import type { Line } from '$lib/utils/fileSections';
	import type { ContentSection } from '$lib/utils/fileSections';
	import type { Hunk } from '$lib/vbranches/types';

	export let lines: Line[];
	export let sectionType: SectionType;
	export let filePath: string;
	export let minWidth = 1.75;
	export let selectable: boolean = false;
	export let selected: boolean = true;
	export let readonly: boolean = false;
	export let draggingDisabled: boolean = false;
	export let tabSize = 4;
	export let hunk: Hunk;
	export let subsection: ContentSection;

	$: isSelected = selectable && selected;
	$: popupMenu = updateContextMenu(filePath);

	const project = getContext(Project);

	const dispatch = createEventDispatcher<{
		selected: boolean;
	}>();

	function updateContextMenu(filePath: string) {
		if (popupMenu) popupMenu.$destroy();
		return new HunkContextMenu({
			target: document.body,
			props: { projectPath: project.vscodePath, filePath, readonly }
		});
	}

	function toTokens(inputLine: string): string[] {
		function sanitize(text: string) {
			var element = document.createElement('div');
			element.innerText = text;
			return element.innerHTML;
		}

		let highlighter = create(inputLine, filePath);
		let tokens: string[] = [];
		highlighter.highlight((text, classNames) => {
			const token = classNames
				? `<span class=${classNames}>${sanitize(text)}</span>`
				: sanitize(text);

			tokens.push(token);
		});
		return tokens;
	}

	onDestroy(() => {
		if (popupMenu) {
			popupMenu.$destroy();
		}
	});
</script>

<div
	class="line-wrapper"
	style="--tab-size: {tabSize}; --minwidth: {minWidth}rem; --cursor: {draggingDisabled
		? 'default'
		: 'grab'}"
>
	{#each lines as line (`${line.afterLineNumber}_${line.content}`)}
		<div
			class="code-line"
			role="group"
			on:contextmenu={(e) => {
				const lineNumber = line.afterLineNumber
					? line.afterLineNumber
					: line.beforeLineNumber;
				popupMenu.openByMouse(e, {
					hunk,
					lineNumber,
					section: subsection
				});
			}}
		>
			<div class="code-line__numbers-line">
				<button
					on:click={() => selectable && dispatch('selected', !selected)}
					class="numbers-line-count"
					class:selected={isSelected}
				>
					{line.beforeLineNumber || ''}
				</button>
				<button
					on:click={() => selectable && dispatch('selected', !selected)}
					class="numbers-line-count"
					class:selected={isSelected}
				>
					{line.afterLineNumber || ''}
				</button>
			</div>
			<div
				class="line"
				class:readonly
				class:diff-line-deletion={sectionType === SectionType.RemovedLines}
				class:diff-line-addition={sectionType === SectionType.AddedLines}
			>
				<span class="selectable-wrapper" data-no-drag>
					{@html toTokens(line.content).join('')}
				</span>
			</div>
		</div>
	{/each}
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
		cursor: var(--cursor);
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
		min-width: var(--minwidth);
		cursor: var(--cursor);

		&.selected {
			background-color: var(--hunk-line-selected-bg);
			border-color: var(--hunk-line-selected-border);
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
