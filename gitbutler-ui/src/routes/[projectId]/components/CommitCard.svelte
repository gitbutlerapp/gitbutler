<script lang="ts">
	import { open } from '@tauri-apps/api/shell';
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
	import Tag from '../components/Tag.svelte';
	import { draggableCommit, nonDraggable } from '$lib/draggables';
	import { draggable } from '$lib/utils/draggable';

	export let commit: Commit | RemoteCommit;
	export let projectId: string;
	export let commitUrl: string | undefined = undefined;
	export let isHeadCommit: boolean = false;
	export let resetHeadCommit: () => void | undefined = () => undefined;
	export let readonly = false;

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

	function onClick() {
		loadEntries();
		previewCommitModal.show();
	}
</script>

<div
	on:click={onClick}
	on:keyup={onClick}
	use:draggable={commit instanceof Commit
		? draggableCommit(commit.branchId, commit)
		: nonDraggable()}
	role="button"
	tabindex="0"
>
	<div class="commit__card" class:is-head-commit={isHeadCommit}>
		<div class="commit__header">
			<span class="commit__description text-base-12 truncate">
				{commit.description}
			</span>
			{#if isHeadCommit && !readonly}
				<Tag
					color="ghost"
					icon="undo-small"
					border
					clickable
					on:click={(e) => {
						e.stopPropagation();
						resetHeadCommit();
					}}>Undo</Tag
				>
			{/if}
		</div>

		<div class="commit__details">
			<div class="commit__author">
				<img
					class="commit__avatar"
					title="Gravatar for {commit.author.email}"
					alt="Gravatar for {commit.author.email}"
					srcset="{commit.author.gravatarUrl} 2x"
					width="100"
					height="100"
					on:error
				/>
				<span class="commit__author-name text-base-12 truncate">{commit.author.name}</span>
			</div>
			<span class="commit__time text-base-11">
				<TimeAgo date={commit.createdAt} />
			</span>
		</div>
	</div>
</div>

<Modal width="large" bind:this={previewCommitModal} icon="commit" title={commit.description}>
	<svelte:fragment slot="header_controls">
		{#if !commit.isLocal && commitUrl}
			<Button
				color="neutral"
				kind="outlined"
				icon="open-link"
				on:click={() => {
					if (commitUrl) open(commitUrl);
				}}>Open commit</Button
			>
		{/if}
	</svelte:fragment>

	<div class="commit-modal__body">
		{#if isLoading}
			<div class="flex w-full justify-center">
				<div class="border-gray-900 h-8 w-8 animate-spin rounded-full border-b-2" />
			</div>
		{:else}
			{#each entries as [filepath, sections]}
				<div class="commit-modal__file-section">
					<div
						class="text-color-3 flex flex-grow items-center overflow-hidden text-ellipsis whitespace-nowrap font-bold"
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
					<div class="commit-modal__code-container custom-scrollbar">
						<div class="commit-modal__code-wrapper">
							{#each sections as section}
								{#if 'hunk' in section}
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
								{/if}
							{/each}
						</div>
					</div>
				</div>
			{/each}
		{/if}
	</div>

	<svelte:fragment slot="controls">
		<Button color="primary" on:click={() => previewCommitModal.close()}>Close</Button>
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

	.commit__card {
		display: flex;
		flex-direction: column;
		cursor: default;
		gap: var(--space-10);
		padding: var(--space-12);
		border-radius: var(--space-6);
		background-color: var(--clr-theme-container-light);
		border: 1px solid var(--clr-theme-container-outline-light);
		transition: background-color var(--transition-fast);

		&:hover {
			border: 1px solid var(--clr-theme-container-outline-pale);
		}
	}

	.commit__header {
		display: flex;
		align-items: center;
		gap: var(--space-8);
	}

	.commit__description {
		flex: 1;
		display: block;
		color: var(--clr-theme-scale-ntrl-0);
		line-height: 120%;
		width: 100%;
	}

	.commit__details {
		display: flex;
		align-items: center;
		gap: var(--space-8);
	}

	.commit__author {
		display: block;
		flex: 1;
		display: flex;
		align-items: center;
		gap: var(--space-6);
	}

	.commit__avatar {
		width: var(--space-16);
		height: var(--space-16);
		border-radius: 100%;
	}

	.commit__author-name {
		max-width: calc(100% - var(--space-16));
	}

	.commit__time,
	.commit__author-name {
		color: var(--clr-theme-scale-ntrl-50);
	}

	.is-head-commit {
		gap: var(--space-6);
	}

	/* modal */
	.commit-modal__code-container {
		display: flex;
		flex-direction: column;
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-theme-container-outline-light);
		overflow-x: auto;
		overflow-y: hidden;
		user-select: text;
	}

	.commit-modal__code-wrapper {
		display: flex;
		flex-direction: column;
		width: 100%;
		min-width: max-content;
	}

	.commit-modal__file-section {
		display: flex;
		flex-direction: column;
		gap: var(--space-12);
	}

	.commit-modal__body {
		display: flex;
		flex-direction: column;
		gap: var(--space-20);
	}
</style>
