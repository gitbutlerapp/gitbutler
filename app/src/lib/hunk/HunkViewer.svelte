<script lang="ts">
	import { Project } from '$lib/backend/projects';
	// import { draggableElement } from '$lib/dragging/draggable';
	// import { DraggableHunk } from '$lib/dragging/draggables';
	import HunkContextMenu from '$lib/hunk/HunkContextMenu.svelte';
	// import HunkLines from '$lib/hunk/HunkLines.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import LargeDiffMessage from '$lib/shared/LargeDiffMessage.svelte';
	// import Scrollbar from '$lib/shared/Scrollbar.svelte';
	import { create } from '$lib/utils/codeHighlight';
	import { getContext, getContextStoreBySymbol, maybeGetContextStore } from '$lib/utils/context';
	import { SectionType } from '$lib/utils/fileSections';
	import { type HunkSection, type ContentSection } from '$lib/utils/fileSections';
	import { Ownership } from '$lib/vbranches/ownership';
	import { VirtualBranch, type Hunk } from '$lib/vbranches/types';
	import { diff_match_patch } from 'diff-match-patch';
	import type { Writable } from 'svelte/store';
	// import ListItem from '$lib/shared/ListItem.svelte';

	const enum RowType {
		Deletion = 'deletion',
		Addition = 'addition',
		Equal = 'equal',
		Spacer = 'spacer'
	}

	interface Token {
		text: string;
		className: string;
	}

	interface Row {
		originalLineNumber?: number;
		currentLineNumber?: number;
		tokens: string[];
		// tokens: Token[];
		type: SectionType;
		size: number;
	}

	interface Props {
		filePath: string;
		section: HunkSection;
		minWidth: number;
		selectable: boolean;
		isUnapplied: boolean;
		isFileLocked: boolean;
		readonly: boolean;
		linesModified: number;
	}

	enum Operation {
		Equal = 0,
		Insert = 1,
		Delete = -1,
		Edit = 2
	}

	type DiffArray = { 0: Operation; 1: string[] }[];

	let {
		filePath,
		section,
		linesModified,
		minWidth,
		selectable = false,
		isUnapplied,
		isFileLocked,
		readonly = false
	}: Props = $props();

	$inspect('section', section);

	// const selectedOwnership: Writable<Ownership> | undefined = maybeGetContextStore(Ownership);
	// const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	// const branch = maybeGetContextStore(VirtualBranch);
	// const project = getContext(Project);

	let alwaysShow = $state(false);
	// let contents = $state<HTMLDivElement>();
	// let viewport = $state<HTMLDivElement>();
	// let contextMenu = $state<HunkContextMenu>();
	// const draggingDisabled = $derived(readonly || isUnapplied);

	// function onHunkSelected(hunk: Hunk, isSelected: boolean) {
	// 	if (!selectedOwnership) return;
	// 	if (isSelected) {
	// 		selectedOwnership.update((ownership) => ownership.add(hunk.filePath, hunk));
	// 	} else {
	// 		selectedOwnership.update((ownership) => ownership.remove(hunk.filePath, hunk.id));
	// 	}
	// }

	function charDiff(text1: string, text2: string): { 0: number; 1: string }[] {
		const differ = new diff_match_patch();
		const diff = differ.diff_main(text1, text2);
		// differ.diff_cleanupSemantic(diff);
		return diff;
	}

	function sanitize(text: string) {
		var element = document.createElement('div');
		element.innerText = text;
		return element.innerHTML;
	}

	function createRowData(section: ContentSection): Row[] {
		return section.lines.map((line) => {
			return {
				originalLineNumber: line.beforeLineNumber,
				currentLineNumber: line.afterLineNumber,
				tokens: toTokens(line.content),
				type: section.sectionType,
				size: line.content.length
			};
		});
	}

	function toTokens(inputLine: string): string[] {
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

	function calculateWordDiff(prevSection: ContentSection, nextSection: ContentSection): Row[] {
		const numberOfLines = nextSection.lines.length;
		const returnRows: Row[] = [];

		// Loop through every line in the section
		// We're only bothered with prev/next sections with equal # of lines changes
		for (let i = 0; i < numberOfLines; i++) {
			const oldLine = prevSection.lines[i];
			const newLine = nextSection.lines[i];
			const nextSectionRow = {
				originalLineNumber: newLine.beforeLineNumber,
				currentLineNumber: newLine.afterLineNumber,
				tokens: [] as string[],
				type: nextSection.sectionType,
				size: newLine.content.length
			};

			const diff = charDiff(oldLine.content, newLine.content);
			console.log('differ', { diff });

			for (const token of diff) {
				const text = token[1];
				const type = token[0];

				if (type === Operation.Equal) {
					nextSectionRow.tokens.push(...toTokens(text));
				} else if (type === Operation.Insert) {
					nextSectionRow.tokens.push(`<span class="token-inserted">${text}</span>`);
				} else if (type === Operation.Delete) {
					nextSectionRow.tokens.push(`<span class="token-deleted">${text}</span>`);
				}
			}
			returnRows.push(nextSectionRow);

			// let forwardRmDiff: DiffData[] = [];
			// let forwardAddDiff: DiffData[] = [];
			//
			// // Ignore whitespace changes
			// const wordRegExp = new RegExp(/\w/g);
			// const oldWordContent = oldLine.content.match(wordRegExp);
			// const newWordContent = newLine.content.match(wordRegExp);
			//
			// // Brute force walk all "word" characters and calculate the
			// // text difference between line0 and line1
			// // TODO: See if we can merge these loops; save on compute
			// oldWordContent?.forEach((char, i) => {
			// 	if (newWordContent?.[i] !== char) {
			// 		forwardRmDiff.push({
			// 			content: char,
			// 			start: i
			// 		});
			// 	}
			// });
			// newWordContent?.forEach((char, i) => {
			// 	if (oldWordContent?.[i] !== char) {
			// 		forwardAddDiff.push({
			// 			content: char,
			// 			start: i
			// 		});
			// 	}
			// });
			//
			// // @ts-expect-error - TODO: Update data structure / types
			// prevSection.lines[i].diffs = {
			// 	removed: forwardRmDiff,
			// 	added: forwardAddDiff
			// };
			// // @ts-expect-error - TODO: Update data structure / types
			// nextSection.lines[i].diffs = {
			// 	removed: forwardRmDiff,
			// 	added: forwardAddDiff
			// };
		}
		console.log({ returnRows });

		return returnRows;
	}

	function filterRows(subsections: ContentSection[]) {
		// Filter out section for which we don't need to compute word diffs
		// in order to save on compute
		return subsections.flatMap((nextSection, i) => {
			const prevSection = subsections[i - 1];
			// If there's no prevLine (first line) or the target line is of type Context (not add or rm), skip
			if (!prevSection || nextSection.sectionType === SectionType.Context)
				return createRowData(nextSection);
			// Skip sections which aren't the same length in lines changed
			if (prevSection.lines.length !== nextSection.lines.length) return createRowData(nextSection);

			return calculateWordDiff(prevSection, nextSection);
		});
	}

	const renderRows = $derived(filterRows(section.subSections));
</script>

<div class="scrollable">
	<div tabindex="0" role="cell">
		<div class="hunk__bg-stretch">
			{#if linesModified > 2500 && !alwaysShow}
				<LargeDiffMessage
					on:show={() => {
						alwaysShow = true;
					}}
				/>
			{:else}
				{@const hunk = section.hunk}
				<table data-hunk-hash={hunk.hash} class="table__section">
					<tbody>
						<tr>
							<td colspan="3">{filePath}</td>
						</tr>
						{#each renderRows as line}
							<tr>
								<td class="table__numberColumn" align="center">{line.originalLineNumber}</td>
								<td class="table__numberColumn" align="center">{line.currentLineNumber}</td>
								<td
									class="table__textContent"
									class:diff-line-deletion={line.type === SectionType.RemovedLines}
									class:diff-line-addition={line.type === SectionType.AddedLines}
								>
									<span class="blob-code-content">
										{@html line.tokens.join('')}
									</span>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			{/if}
		</div>
	</div>
</div>

<style lang="postcss">
	.scrollable {
		display: flex;
		flex-direction: column;
		position: relative;
		border-radius: var(--radius-s);
		overflow-x: scroll;

		& > div {
			width: 100%;
		}
	}

	.hunk {
		display: flex;
		flex-direction: column;
		overflow-x: auto;
		user-select: text;

		background: var(--clr-bg-1);
		border-radius: var(--radius-s);
		border: 1px solid var(--clr-border-2);
		transition: border-color var(--transition-fast);
	}

	.hunk__bg-stretch {
		width: 100%;
		min-width: max-content;
	}

	.table__section {
		width: 100%;
	}

	.table__numberColumn {
		padding-inline: 0.35rem;
	}

	.table__textContent {
	}

	.diff-line-deletion {
		background-color: #cf8d8e20;
	}

	.diff-line-addition {
		background-color: #94cf8d20;
	}

	.blob-code-content {
		font-family: 'monospace';
		white-space: pre;
		user-select: text;

		&:hover {
			cursor: text;
		}
	}
</style>
