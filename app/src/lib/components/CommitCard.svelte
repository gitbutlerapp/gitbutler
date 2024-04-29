<script lang="ts">
	import BranchFilesList from './BranchFilesList.svelte';
	import { Project } from '$lib/backend/projects';
	import Tag from '$lib/components/Tag.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import { persistedCommitMessage } from '$lib/config/config';
	import { draggable } from '$lib/dragging/draggable';
	import { DraggableCommit, nonDraggable } from '$lib/dragging/draggables';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { createCommitStore, getSelectedFiles } from '$lib/vbranches/contexts';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { listRemoteCommitFiles } from '$lib/vbranches/remoteCommits';
	import { RemoteCommit, Commit, RemoteFile, Branch, BaseBranch } from '$lib/vbranches/types';
	import { slide } from 'svelte/transition';

	export let branch: Branch | undefined = undefined;
	export let commit: Commit | RemoteCommit;
	export let commitUrl: string | undefined = undefined;
	export let isHeadCommit: boolean = false;
	export let isUnapplied = false;

	const branchController = getContext(BranchController);
	const baseBranch = getContextStore(BaseBranch);
	const project = getContext(Project);
	const selectedFiles = getSelectedFiles();
	const fileIdSelection = getContext(FileIdSelection);

	const commitStore = createCommitStore(commit);
	$: commitStore.set(commit);

	const currentCommitMessage = persistedCommitMessage(project.id, branch?.id || '');

	let showFiles = false;
	let files: RemoteFile[] = [];

	$: selectedFile =
		$fileIdSelection.length == 1 &&
		fileIdSelection.only().commitId == commit.id &&
		files.find((f) => f.id == fileIdSelection.only().fileId);
	$: if (selectedFile) selectedFiles.set([selectedFile]);

	async function loadFiles() {
		files = await listRemoteCommitFiles(project.id, commit.id);
	}

	function toggleFiles() {
		showFiles = !showFiles;
		if (!showFiles && branch) {
			if (commit.description != description) {
				branchController.updateCommitMessage(branch.id, commit.id, description);
			}
		}
		if (showFiles) loadFiles();
	}

	function onKeyup(e: KeyboardEvent) {
		if (e.key == 'Enter' || e.key == ' ') {
			toggleFiles();
		}
	}

	function undoCommit(commit: Commit | RemoteCommit) {
		if (!branch || !$baseBranch) {
			console.error('Unable to undo commit');
			return;
		}
		branchController.undoCommit(branch.id, commit.id);
	}

	function insertBlankCommit(commit: Commit | RemoteCommit, offset: number) {
		if (!branch || !$baseBranch) {
			console.error('Unable to insert commit');
			return;
		}
		branchController.insertBlankCommit(branch.id, commit.id, offset);
	}

	function reorderCommit(commit: Commit | RemoteCommit, offset: number) {
		if (!branch || !$baseBranch) {
			console.error('Unable to move commit');
			return;
		}
		branchController.reorderCommit(branch.id, commit.id, offset);
	}

	const isUndoable = !isUnapplied;
	const hasCommitUrl = !commit.isLocal && commitUrl;
	let description = commit.description;
</script>

<div
	use:draggable={commit instanceof Commit
		? {
				data: new DraggableCommit(commit.branchId, commit, isHeadCommit)
			}
		: nonDraggable()}
	class="commit"
	class:is-commit-open={showFiles}
>
	{#if isUndoable && showFiles}
		<div class="commit__edit_description">
			<textarea
				placeholder="commit message here"
				bind:value={description}
				rows={commit.description.split('\n').length + 1}
			></textarea>
		</div>
	{/if}
	<div class="commit__header" on:click={toggleFiles} on:keyup={onKeyup} role="button" tabindex="0">
		<div class="commit__message">
			{#if !showFiles}
				<div class="commit__id">
					<code>{commit.id.substring(0, 6)}</code>
				</div>
			{/if}
			<div class="commit__row">
				{#if isUndoable}
					{#if !showFiles}
						{#if commit.descriptionTitle}
							<span class="commit__title text-semibold text-base-12" class:truncate={!showFiles}>
								{commit.descriptionTitle}
							</span>
						{:else}
							<span
								class="commit__title_no_desc text-base-12 text-zinc-400"
								class:truncate={!showFiles}
							>
								<i>empty commit message</i>
							</span>
						{/if}
						<Tag
							style="ghost"
							kind="solid"
							icon="undo-small"
							clickable
							on:click={(e) => {
								currentCommitMessage.set(commit.description);
								e.stopPropagation();
								undoCommit(commit);
							}}>Undo</Tag
						>
					{/if}
				{:else}
					<span class="commit__title text-base-12" class:truncate={!showFiles}>
						{commit.descriptionTitle}
					</span>
				{/if}
			</div>
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
			<BranchFilesList {files} {isUnapplied} />
		</div>

		{#if hasCommitUrl || isUndoable}
			<div class="files__footer">
				{#if isUndoable}
					<Tag
						style="ghost"
						kind="solid"
						clickable
						on:click={(e) => {
							e.stopPropagation();
							reorderCommit(commit, -1);
						}}>Move Up</Tag
					>
					<Tag
						style="ghost"
						kind="solid"
						clickable
						on:click={(e) => {
							e.stopPropagation();
							reorderCommit(commit, 1);
						}}>Move Down</Tag
					>
					<Tag
						style="ghost"
						kind="solid"
						clickable
						on:click={(e) => {
							e.stopPropagation();
							insertBlankCommit(commit, -1);
						}}>Add Before</Tag
					>
					<Tag
						style="ghost"
						kind="solid"
						clickable
						on:click={(e) => {
							e.stopPropagation();
							insertBlankCommit(commit, 1);
						}}>Add After</Tag
					>
					<Tag
						style="ghost"
						kind="solid"
						icon="undo-small"
						clickable
						on:click={(e) => {
							currentCommitMessage.set(commit.description);
							e.stopPropagation();
							undoCommit(commit);
						}}>Undo</Tag
					>
				{/if}
				{#if hasCommitUrl}
					<Tag
						style="ghost"
						kind="solid"
						icon="open-link"
						clickable
						on:click={() => {
							if (commitUrl) openExternalUrl(commitUrl);
						}}>Open commit</Tag
					>
				{/if}
			</div>
		{/if}
	{/if}
</div>

<style lang="postcss">
	/* amend drop zone */
	:global(.amend-dz-active .amend-dz-marker) {
		display: flex;
	}
	:global(.amend-dz-hover .hover-text) {
		visibility: visible;
	}

	.commit {
		display: flex;
		flex-direction: column;

		border-radius: var(--size-6);
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		overflow: hidden;
		transition: background-color var(--transition-fast);

		&:not(.is-commit-open):hover {
			background-color: var(--clr-bg-2);
		}
	}

	.commit__header {
		cursor: pointer;
		display: flex;
		flex-direction: column;
		gap: var(--size-10);
		padding: var(--size-14);
	}

	.is-commit-open {
		background-color: var(--clr-bg-2);

		& .commit__header {
			padding-bottom: var(--size-16);
			border-bottom: 1px solid var(--clr-border-2);
		}

		& .commit__message {
			margin-bottom: var(--size-4);
		}
	}

	.commit__message {
		display: flex;
		flex-direction: column;
		gap: var(--size-6);
	}

	.commit__title {
		flex: 1;
		display: block;
		color: var(--clr-scale-ntrl-0);
		width: 100%;
	}
	.commit__title_no_desc {
		flex: 1;
		display: block;
		width: 100%;
	}

	.commit__body {
		flex: 1;
		display: block;
		width: 100%;
		color: var(--clr-scale-ntrl-40);
		white-space: pre-line;
		word-wrap: anywhere;
	}

	.commit__row {
		display: flex;
		align-items: center;
		gap: var(--size-8);
	}

	.commit__edit_description {
		width: 100%;
	}
	.commit__edit_description textarea {
		width: 100%;
		padding: 10px 14px;
		margin: 0;
		border-bottom: 1px solid #dddddd;
	}

	.commit__id {
		display: flex;
		align-items: center;
		justify-content: center;
		margin-top: -14px;
	}
	.commit__id > code {
		background-color: #eeeeee;
		padding: 1px 12px;
		color: #888888;
		font-size: x-small;
		border-radius: 0px 0px 6px 6px;
		margin-bottom: -8px;
	}

	.commit__author {
		display: block;
		flex: 1;
		display: flex;
		align-items: center;
		gap: var(--size-6);
	}

	.commit__avatar {
		width: var(--size-16);
		height: var(--size-16);
		border-radius: 100%;
	}

	.commit__author-name {
		max-width: calc(100% - var(--size-16));
	}

	.commit__time,
	.commit__author-name {
		color: var(--clr-scale-ntrl-50);
	}

	.files-container {
		background-color: var(--clr-bg-1);
		padding: 0 var(--size-14) var(--size-14);
	}

	.files__footer {
		display: flex;
		justify-content: flex-end;
		gap: var(--size-8);
		padding: var(--size-14);
		background-color: var(--clr-bg-1);
		border-top: 1px solid var(--clr-border-2);
	}
</style>
