<script lang="ts">
	import { create } from '$lib/components/Differ/CodeHighlighter';
	import { SectionType } from '$lib/utils/fileSections';
	import type { Line } from '$lib/utils/fileSections';

	interface Props {
		lines: Line[];
		sectionType: SectionType;
		filePath: string;
		minWidth: number;
		selectable: boolean;
		selected: boolean;
		readonly: boolean;
		draggingDisabled: boolean;
		tabSize: number;

		handleSelected: (isSelected: boolean) => void;
		handleClick: () => void;
		handleLineContextMenu: ({
			lineNumber,
			event
		}: {
			lineNumber: number | undefined;
			event: MouseEvent;
		}) => void;
	}

	const {
		lines,
		sectionType,
		filePath,
		minWidth = 1.75,
		selectable = false,
		selected = true,
		readonly = false,
		draggingDisabled = false,
		tabSize = 4,

		handleSelected,
		handleClick,
		handleLineContextMenu
	}: Props = $props();

	$inspect('lines', lines);

	function toTokens(inputLine: string): string[] {
		// debugger;
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

	let isSelected = $derived(selectable && selected);
</script>

<div
	class="line-wrapper"
	style="--tab-size: {tabSize}; --minwidth: {minWidth}rem; --cursor: {draggingDisabled
		? 'default'
		: 'grab'}"
>
	{#each lines as line}
		<div
			tabindex="-1"
			role="none"
			class="code-line"
			onclick={handleClick}
			oncontextmenu={(event) => {
				const lineNumber = line.afterLineNumber ? line.afterLineNumber : line.beforeLineNumber;
				handleLineContextMenu({ event, lineNumber });
			}}
		>
			<div class="code-line__numbers-line">
				<button
					onclick={() => selectable && handleSelected(!selected)}
					class="numbers-line-count"
					class:selected={isSelected}
				>
					{line.beforeLineNumber || ''}
				</button>
				<button
					onclick={() => selectable && handleSelected(!selected)}
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

		font-size: 11px;
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
		font-size: 10px;
		flex-shrink: 0;
		user-select: none;
		border-right-width: 1px;
		padding-left: 2px;
		padding-right: 2px;
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
		text-indent: 4px;
		margin-right: 4px;
	}
</style>
