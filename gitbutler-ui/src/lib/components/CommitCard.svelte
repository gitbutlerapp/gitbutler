<script lang="ts">
	import BranchFiles from './BranchFiles.svelte';
	import Button from '$lib/components/Button.svelte';
	import Tag from '$lib/components/Tag.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import { projectCurrentCommitMessage } from '$lib/config/config';
	import { draggable } from '$lib/dragging/draggable';
	import { draggableCommit, nonDraggable } from '$lib/dragging/draggables';
	import { openExternalUrl } from '$lib/utils/url';
	import { Ownership } from '$lib/vbranches/ownership';
	import { listRemoteCommitFiles } from '$lib/vbranches/remoteCommits';
	import { LocalFile, RemoteCommit, Commit, RemoteFile } from '$lib/vbranches/types';
	import { writable, type Writable } from 'svelte/store';
	import { slide } from 'svelte/transition';
	import type { BranchController } from '$lib/vbranches/branchController';

	export let commit: Commit | RemoteCommit;
	export let projectId: string;
	export let commitUrl: string | undefined = undefined;
	export let isHeadCommit: boolean = false;
	export let resetHeadCommit: () => void | undefined = () => undefined;
	export let isUnapplied = false;
	export let selectedFiles: Writable<(LocalFile | RemoteFile)[]>;
	export let branchController: BranchController;
	export let branchId: string | undefined = undefined;

	const selectedOwnership = writable(Ownership.default());
	const currentCommitMessage = projectCurrentCommitMessage(projectId, branchId || '');

	let showFiles = false;

	let files: RemoteFile[] = [];

	async function loadFiles() {
		files = await listRemoteCommitFiles(projectId, commit.id);
	}

	function onClick() {
		showFiles = !showFiles;
		if (showFiles) loadFiles();
	}
</script>

<div
	use:draggable={commit instanceof Commit
		? draggableCommit(commit.branchId, commit)
		: nonDraggable()}
	class="commit"
	class:is-commit-open={showFiles}
>
	<div class="commit__header" on:click={onClick} on:keyup={onClick} role="button" tabindex="0">
		<div class="commit__row">
			<span class="commit__description text-base-12 truncate">
				{commit.description}
			</span>
			{#if isHeadCommit && !isUnapplied}
				<Tag
					color="ghost"
					icon="undo-small"
					border
					clickable
					on:click={(e) => {
						currentCommitMessage.set(commit.description);
						e.stopPropagation();
						resetHeadCommit();
					}}>Undo</Tag
				>
			{/if}
		</div>
		<div class="commit__row">
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

	{#if showFiles}
		<div class="files-container" transition:slide={{ duration: 100 }}>
			<BranchFiles
				branchId="blah"
				{files}
				{isUnapplied}
				{selectedOwnership}
				{selectedFiles}
				{branchController}
				allowMultiple={true}
				readonly={true}
			/>

			{#if !commit.isLocal && commitUrl}
				<div class="files__footer">
					<Button
						color="neutral"
						kind="outlined"
						icon="open-link"
						on:click={() => {
							if (commitUrl) openExternalUrl(commitUrl);
						}}>Open commit</Button
					>
				</div>
			{/if}
		</div>
	{/if}
</div>

<style lang="postcss">
	/* amend drop zone */
	:global(.amend-dz-active .amend-dz-marker) {
		@apply flex;
	}
	:global(.amend-dz-hover .hover-text) {
		@apply visible;
	}

	.commit {
		display: flex;
		flex-direction: column;
		cursor: default;
		border-radius: var(--space-6);
		background-color: var(--clr-theme-container-light);
		border: 1px solid var(--clr-theme-container-outline-light);
		overflow: hidden;
		transition: background-color var(--transition-fast);

		&:not(.is-commit-open):hover {
			border: 1px solid
				color-mix(in srgb, var(--clr-theme-container-outline-light), var(--darken-tint-mid));
			background-color: color-mix(
				in srgb,
				var(--clr-theme-container-light),
				var(--darken-tint-extralight)
			);
		}
	}

	.commit__header {
		display: flex;
		flex-direction: column;
		gap: var(--space-10);
		padding: var(--space-12);
	}

	.is-commit-open {
		background-color: color-mix(
			in srgb,
			var(--clr-theme-container-light),
			var(--darken-tint-light)
		);

		& .commit__header {
			padding-bottom: var(--space-16);

			&:hover {
				background-color: color-mix(
					in srgb,
					var(--clr-theme-container-light),
					var(--darken-tint-mid)
				);
			}
		}
	}

	.commit__description {
		flex: 1;
		display: block;
		color: var(--clr-theme-scale-ntrl-0);
		line-height: 120%;
		width: 100%;
	}

	.commit__row {
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

	.files-container {
		background-color: var(--clr-theme-container-light);
	}

	.files__footer {
		text-align: right;
		padding: var(--space-12);
		border-top: 1px solid var(--clr-theme-container-outline-light);
	}
</style>
