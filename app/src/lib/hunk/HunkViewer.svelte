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
	import type { Writable } from 'svelte/store';
	// import ListItem from '$lib/shared/ListItem.svelte';

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

	let {
		filePath,
		section,
    linesModified,
		minWidth,
		selectable = false,
		isUnapplied,
		isFileLocked,
		readonly = false,
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

	// const subsections = $derived(
	// 	section.subSections.flatMap((subsection) => {
	// 		return subsection.lines.flatMap((line) => ({
	// 			...line,
	// 			sectionType: subsection.sectionType
	// 		}));
	// 	})
	// );

	// $inspect(subsections);

	function toTokens(inputLine: string): string[] {
		// debugger;
		function sanitize(text: string) {
			var element = document.createElement('div');
			element.innerText = text;
			return element.innerHTML;
		}

		let highlighter = create(inputLine, filePath);
    console.log('HIGHLIGHTER', highlighter)
		let tokens: string[] = [];
		highlighter.highlight((text, classNames) => {
			const token = classNames
				? `<span class=${classNames}>${sanitize(text)}</span>`
				: sanitize(text);

			tokens.push(token);
		});
		return tokens;
	}

	function filterSections(subsections: ContentSection[]): ContentSection[] {
		return subsections.map((nextSection, i) => {
			const prevSection = subsections[i - 1];
			if (!prevSection || nextSection.sectionType === SectionType.Context) return nextSection;

			nextSection = calculateWordDiff(prevSection, nextSection);

			return nextSection;
		});
	}

	const subSections = $derived(filterSections(section.subSections));

	function calculateWordDiff(
		prevSection: ContentSection,
		nextSection: ContentSection
	): ContentSection {
		// Skip sections which aren't the same length in lines changed
		if (prevSection.lines.length !== nextSection.lines.length) return nextSection;
		const numberOfLines = nextSection.lines.length;

    // Loop through every line in the section
    // We're only bothered with prev/next sections with equal # of lines changes
		for (let i = 0; i < numberOfLines; i++) {
			const oldLine = prevSection.lines[i];
			const newLine = nextSection.lines[i];
			let forwardRmDiff: string[] = [];
			let forwardAddDiff: string[] = [];

      // Ignore whitespace changes
			const wordRegExp = new RegExp(/\w/g);
			const oldWordContent = oldLine.content.match(wordRegExp);
			const newWordContent = newLine.content.match(wordRegExp);

      // Brute force walk all "word" characters and calculate the 
      // text difference between line0 and line1
      // TODO: See if we can merge these loops; save on compute
			oldWordContent?.forEach((char, i) => {
				if (newWordContent?.[i] !== char) {
					forwardRmDiff.push(char);
				}
			});
			newWordContent?.forEach((char, i) => {
				if (oldWordContent?.[i] !== char) {
					forwardAddDiff.push(char);
				}
			});

      // @ts-expect-error - TODO: Update data structure / types
			prevSection.lines[i].diffs = {
        removed: forwardRmDiff.join(''),
        added: forwardAddDiff.join('')
      }
		}
		console.log({ prevSection, nextSection });

		return nextSection;
	}
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
						{#each subSections as section}
							{#each section.lines as line}
								<tr>
									<td class="table__numberColumn" align="center">{line.beforeLineNumber}</td>
									<td class="table__numberColumn" align="center">{line.afterLineNumber}</td>
									<td
										class="table__textContent"
										class:diff-line-deletion={section.sectionType === SectionType.RemovedLines}
										class:diff-line-addition={section.sectionType === SectionType.AddedLines}
									>
										<span class="blob-code-content">
											{@html toTokens(line.content)}
										</span>
									</td>
								</tr>
							{/each}
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
