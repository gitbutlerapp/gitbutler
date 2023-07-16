<script lang="ts">
	import { parseFileSections } from './fileSections';
	import type { File } from '$lib/vbranches';
	import RenderedLine from './RenderedLine.svelte';
	export let file: File;

	$: sections = parseFileSections(file);
</script>

<div>
	{#each sections as section}
		{#if 'hunk' in section}
			<div class="border border-dark-100">
				{#each section.subSections as subsection}
					<div
						class="grid h-full w-full flex-auto whitespace-pre font-mono text-sm"
						style:grid-template-columns="minmax(auto, max-content) minmax(auto, max-content) 1fr"
					>
						{#each subsection.lines.slice(0, subsection.linesShown) as line}
							<RenderedLine {line} sectionType={subsection.sectionType} />
						{/each}
					</div>
					{#if subsection.linesShown < subsection.lines.length}
						<button
							class="text-sm"
							on:click={() => {
								subsection.linesShown = subsection.lines.length;
							}}
						>
							Expand {subsection.lines.length} lines
						</button>
					{/if}
				{/each}
			</div>
		{:else}
			<div class="border-l border-transparent">
				<div
					class="grid h-full w-full flex-auto whitespace-pre font-mono text-sm"
					style:grid-template-columns="minmax(auto, max-content) minmax(auto, max-content) 1fr"
				>
					{#each section.lines.slice(0, section.linesShown) as line}
						<RenderedLine {line} sectionType={section.sectionType} />
					{/each}
				</div>
			</div>
			{#if section.linesShown < section.lines.length}
				<button
					class="text-sm"
					on:click={() => {
						if ('linesShown' in section) {
							section.linesShown = section.lines.length;
						}
					}}
				>
					Expand {section.lines.length} lines
				</button>
			{/if}
		{/if}
	{/each}
</div>
