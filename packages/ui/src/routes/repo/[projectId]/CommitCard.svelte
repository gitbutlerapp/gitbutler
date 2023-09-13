<script lang="ts">
	import type { Commit } from '$lib/vbranches/types';
	import TimeAgo from '$lib/components/TimeAgo/TimeAgo.svelte';
	import Tooltip from '$lib/components/Tooltip/Tooltip.svelte';
	import { getCommitDiff } from '$lib/api/git/diffs';
	import { getVSIFileIcon } from '$lib/ext-icons';
	import { ContentSection, HunkSection, parseFileSections } from './fileSections';
	import type { File, Hunk } from '$lib/vbranches/types';
	import RenderedLine from './RenderedLine.svelte';
	import { IconExpandUpDown, IconExpandUp, IconExpandDown } from '$lib/icons';
	import { Button, Modal } from '$lib/components';

	export let commit: Commit;
	export let isIntegrated = false;
	export let url: string | undefined = undefined;
	export let projectId: string;

	let previewCommitModal: Modal;
	let minWidth = 2;
	let fileSections: Map<string, (HunkSection | ContentSection)[]> = new Map();

	function parseDiff(diff: string, filepath: string): (HunkSection | ContentSection)[] {
		let hunkDiffs = diff.split(/(@@.*@@)/).filter((s) => s.trim() !== '');

		hunkDiffs = hunkDiffs.reduce(function (result: string[], value, index, array) {
			if (index % 2 === 0) result.push(array.slice(index, index + 2).join());
			return result;
		}, []);
		const mockDate = new Date();

		let hunks: Hunk[] = hunkDiffs.map((diff) => {
			return {
				id: '',
				diff: diff,
				modifiedAt: mockDate,
				filePath: filepath,
				locked: false
			};
		});

		let file: File = {
			id: '',
			path: filepath,
			hunks: hunks,
			expanded: true,
			conflicted: false,
			binary: false,
			modifiedAt: mockDate,
			content: ''
		};
		return parseFileSections(file);
	}
</script>

<div class="text-color-2 bg-color-5 border-color-4 w-full truncate rounded border p-2 text-left">
	<div class="mb-1 flex justify-between">
		<div class="truncate">
			{#if url}
				<button
					on:click={() => {
						getCommitDiff({ projectId: projectId, commitId: commit.id }).then((result) => {
							let entries = Object.entries(result);

							entries.forEach(([filepath, diff]) => {
								fileSections.set(filepath, parseDiff(diff, filepath));
							});
							previewCommitModal.show();
						});
					}}
				>
					{commit.description}
				</button>
			{:else}
				{commit.description}
			{/if}
		</div>
		{#if isIntegrated}
			<div>
				<Tooltip label="This commit is integrated into Trunk and will dissapear once you merge it.">
					<i>integrated</i>
				</Tooltip>
			</div>
		{/if}
	</div>

	<div class="text-color-3 flex space-x-1 text-sm">
		<img
			class="relative inline-block h-4 w-4 rounded-full ring-1 ring-white dark:ring-black"
			title="Gravatar for {commit.author.email}"
			alt="Gravatar for {commit.author.email}"
			srcset="{commit.author.gravatarUrl} 2x"
			width="100"
			height="100"
			on:error
		/>
		<div class="flex-grow truncate">{commit.author.name}</div>
		<div class="truncate">
			<TimeAgo date={commit.createdAt} />
		</div>
	</div>
</div>

<Modal width="large" bind:this={previewCommitModal}>
	<div class="flex w-full flex-col gap-4">
		{#each fileSections.entries() as [filepath, sections]}
			<div>
				<div
					class="text-color-3 flex flex-grow items-center overflow-hidden text-ellipsis whitespace-nowrap px-2 font-bold"
					title={filepath}
				>
					<img
						src={getVSIFileIcon(filepath)}
						alt="js"
						width="13"
						style="width: 0.8125rem"
						class="mr-1 inline"
					/>

					{filepath}
				</div>
				<div class="flex flex-col rounded px-2">
					{#each sections as section}
						{#if 'hunk' in section}
							<div class="border-color-4 my-1 flex w-full flex-col overflow-hidden rounded">
								<div class="w-full overflow-hidden">
									{#each section.subSections as subsection, sidx}
										{#each subsection.lines.slice(0, subsection.expanded ? subsection.lines.length : 0) as line}
											<RenderedLine
												{line}
												{minWidth}
												sectionType={subsection.sectionType}
												filePath={filepath}
											/>
										{/each}
										{#if !subsection.expanded}
											<div
												class="border-color-4 flex w-full"
												class:border-t={sidx == section.subSections.length - 1 ||
													(sidx > 0 && sidx < section.subSections.length - 1)}
												class:border-b={sidx == 0 ||
													(sidx > 0 && sidx < section.subSections.length - 1)}
											>
												<div
													class="bg-color-4 text-color-4 hover:text-color-2 border-color-4 border-r text-center"
													style:min-width={`calc(${2 * minWidth}rem - 1px)`}
												>
													<button
														class="flex justify-center py-0.5 text-sm"
														style:width={`calc(${2 * minWidth}rem - 1px)`}
														on:click={() => {
															if ('expanded' in subsection) {
																subsection.expanded = true;
															}
														}}
													>
														{#if sidx == 0}
															<IconExpandUp />
														{:else if sidx == section.subSections.length - 1}
															<IconExpandDown />
														{:else}
															<IconExpandUpDown />
														{/if}
													</button>
												</div>
												<div class="bg-color-4 flex-grow" />
											</div>
										{/if}
									{/each}
								</div>
							</div>
						{/if}
					{/each}
				</div>
			</div>
		{/each}
	</div>

	<svelte:fragment slot="controls">
		<div class="px-4">
			<Button color="purple" on:click={() => previewCommitModal.close()}>Close</Button>
		</div>
	</svelte:fragment>
</Modal>
