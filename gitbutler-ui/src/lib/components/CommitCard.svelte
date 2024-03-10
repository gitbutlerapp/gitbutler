<script lang="ts">
	import BranchFiles from './BranchFiles.svelte';
	import Tag from '$lib/components/Tag.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import { projectCurrentCommitMessage } from '$lib/config/config';
	import { draggable } from '$lib/dragging/draggable';
	import { draggableCommit, nonDraggable } from '$lib/dragging/draggables';
	import { openExternalUrl } from '$lib/utils/url';
	import { Ownership } from '$lib/vbranches/ownership';
	import { listRemoteCommitFiles } from '$lib/vbranches/remoteCommits';
	import {
		LocalFile,
		RemoteCommit,
		Commit,
		RemoteFile,
		Branch,
		BaseBranch
	} from '$lib/vbranches/types';
	import { writable, type Writable } from 'svelte/store';
	import { slide } from 'svelte/transition';
	import type { Project } from '$lib/backend/projects';
	import type { BranchController } from '$lib/vbranches/branchController';

	export let branch: Branch | undefined = undefined;
	export let base: BaseBranch | undefined | null = undefined;
	export let project: Project | undefined;
	export let commit: Commit | RemoteCommit;
	export let projectId: string;
	export let commitUrl: string | undefined = undefined;
	export let isHeadCommit: boolean = false;
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

	function resetHeadCommit() {
		if (!branch || !base) {
			console.error('Unable to reset head commit');
			return;
		}
		if (branch.commits.length > 1) {
			branchController.resetBranch(branch.id, branch.commits[1].id);
		} else if (branch.commits.length === 1 && base) {
			branchController.resetBranch(branch.id, base.baseSha);
		}
	}

	const isUndoable = isHeadCommit && !isUnapplied;
	const hasCommitUrl = !commit.isLocal && commitUrl;
</script>

<div
	use:draggable={commit instanceof Commit
		? draggableCommit(commit.branchId, commit, isHeadCommit)
		: nonDraggable()}
	class="commit"
	class:is-commit-open={showFiles}
>
	<div class="commit__header" on:click={onClick} on:keyup={onClick} role="button" tabindex="0">
		<div class="commit__message">
			<div class="commit__row">
				<span class="commit__title text-semibold text-base-12" class:truncate={!showFiles}>
					{commit.descriptionTitle}
				</span>
				{#if isUndoable && !showFiles}
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
			{#if showFiles && commit.descriptionBody}
				<div class="commit__row" transition:slide={{ duration: 100 }}>
					<span class="commit__body text-base-body-12">
						{commit.descriptionBody}
					</span>
				</div>
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
				{project}
				allowMultiple={true}
				readonly={true}
			/>

			{#if hasCommitUrl || isUndoable}
				<div class="files__footer">
					{#if isUndoable}
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
					{#if hasCommitUrl}
						<Tag
							color="ghost"
							icon="open-link"
							border
							clickable
							on:click={() => {
								if (commitUrl) openExternalUrl(commitUrl);
							}}>Open commit</Tag
						>
					{/if}
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
		cursor: pointer;
		display: flex;
		flex-direction: column;
		gap: var(--space-10);
		padding: var(--space-14);
	}

	.is-commit-open {
		background-color: color-mix(
			in srgb,
			var(--clr-theme-container-light),
			var(--darken-tint-extralight)
		);

		& .commit__header {
			padding-bottom: var(--space-16);
			border-bottom: 1px solid var(--clr-theme-container-outline-light);

			&:hover {
				background-color: color-mix(
					in srgb,
					var(--clr-theme-container-light),
					var(--darken-tint-light)
				);
			}
		}

		& .commit__message {
			margin-bottom: var(--space-4);
		}
	}

	.commit__message {
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
	}

	.commit__title {
		flex: 1;
		display: block;
		color: var(--clr-theme-scale-ntrl-0);
		width: 100%;
	}

	.commit__body {
		flex: 1;
		display: block;
		width: 100%;
		color: var(--clr-theme-scale-ntrl-40);
		white-space: pre-line;
		word-wrap: anywhere;
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
		display: flex;
		justify-content: flex-end;
		gap: var(--space-8);
		padding: var(--space-14);
		border-top: 1px solid var(--clr-theme-container-outline-light);
	}
</style>
