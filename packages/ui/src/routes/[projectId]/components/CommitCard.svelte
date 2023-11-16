<script lang="ts">
	import { RemoteFile, type RemoteCommit, Commit } from '$lib/vbranches/types';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import { getVSIFileIcon } from '$lib/ext-icons';
	import { ContentSection, HunkSection, parseFileSections } from './fileSections';
	import RenderedLine from './RenderedLine.svelte';
	import { IconExpandUpDown, IconExpandUp, IconExpandDown } from '$lib/icons';
	import { invoke } from '$lib/backend/ipc';
	import { plainToInstance } from 'class-transformer';
	import Modal from '$lib/components/Modal.svelte';
	import Button from '$lib/components/Button.svelte';
	import Link from '$lib/components/Link.svelte';
	import { draggableCommit, nonDraggable } from '$lib/draggables';
	import { draggable } from '$lib/utils/draggable';

	export let commit: Commit | RemoteCommit;
	export let projectId: string;
	export let commitUrl: string | undefined = undefined;

	let previewCommitModal: Modal;
	let minWidth = 2;

	let entries: [string, (ContentSection | HunkSection)[]][] = [];
	let isLoading = false;
	async function loadEntries() {
		isLoading = true;
		entries = plainToInstance(
			RemoteFile,
			await invoke<any[]>('list_remote_commit_files', { projectId, commitOid: commit.id })
		)
			.map(
				(file) => [file.path, parseFileSections(file)] as [string, (ContentSection | HunkSection)[]]
			)
			.sort((a, b) => a[0].localeCompare(b[0]));
		isLoading = false;
	}
</script>

<div
	class="w-full"
	use:draggable={commit instanceof Commit
		? draggableCommit(commit.branchId, commit)
		: nonDraggable()}
>
	<div
		class="text-color-2 border-color-4 rounded border p-2 text-left"
		style:background-color="var(--bg-card)"
		style:border-color="var(--border-card)"
	>
		<div class="mb-1">
			<button
				class="max-w-full overflow-hidden truncate"
				on:click={() => {
					loadEntries();
					previewCommitModal.show();
				}}
			>
				{commit.description}
			</button>
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
			<div class="flex-1 truncate">{commit.author.name}</div>
			<div class="truncate">
				<TimeAgo date={commit.createdAt} />
			</div>
		</div>
	</div>
</div>

<Modal width="large" bind:this={previewCommitModal}>
	<div class="flex w-full flex-col gap-4 overflow-x-hidden">
		<div>
			<Link target="_blank" rel="noreferrer" href={commitUrl} class="text-3">
				{commit.description}
			</Link>
		</div>
		<div class="overflow-y-scroll">
			{#if isLoading}
				<div class="flex w-full justify-center">
					<div class="border-gray-900 h-32 w-32 animate-spin rounded-full border-b-2" />
				</div>
			{:else}
				{#each entries as [filepath, sections]}
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
			{/if}
		</div>
	</div>

	<svelte:fragment slot="controls">
		<div class="px-4">
			<Button color="purple" on:click={() => previewCommitModal.close()}>Close</Button>
		</div>
	</svelte:fragment>
</Modal>

<style lang="postcss">
	/* amend drop zone */
	:global(.amend-dz-active .amend-dz-marker) {
		@apply flex;
	}
	:global(.amend-dz-hover .hover-text) {
		@apply visible;
	}
</style>
