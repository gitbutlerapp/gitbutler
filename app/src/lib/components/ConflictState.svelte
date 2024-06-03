<script lang="ts" async="true">
	import { Project } from '$lib/backend/projects';
	import { getContext } from '$lib/utils/context';
	import { listRemoteCommitFiles } from '$lib/vbranches/remoteCommits';
	import { BranchController } from '$lib/vbranches/branchController';
	import Tag from './Tag.svelte';
	import FileCard from './FileCard.svelte';

	export let commit: string;
	export let branch: Branch;

	const branchController = getContext(BranchController);
	const project = getContext(Project);
	let files = [];

	async function loadFiles() {
		files = await listRemoteCommitFiles(project.id, commit);
		console.log(files);
	}
	loadFiles();

	function resolveConflictFinish() {
		branchController.resolveConflictFinish(branch.id, commit);
	}

	function resolveConflictAbandon() {
		branchController.resolveConflictAbandon(branch.id, commit);
	}
</script>

<div class="conflict-box">
	<div class="conflict-box-inner">
		<h1>You Are in Conflict Resolution Mode</h1>
		<div>Go to your editor and solve conflicts in the following files:</div>
		<ul>
			{#each files as file}
				<li>{file.path}</li>
			{/each}
		</ul>
		<hr />
		<div class="title">Actions:</div>
		<div class="actions">
			<Tag style="success" on:click={resolveConflictFinish}>Resolve</Tag>
			<Tag style="error" on:click={resolveConflictAbandon}>Abandon</Tag>
		</div>
		<hr />
		<div>Conflict Commit : <code>{commit}</code></div>
		<div>Branch: <code>{branch.id}</code></div>
	</div>
</div>

<style lang="postcss">
	h1 {
		font-size: 24px;
		font-weight: 500;
		margin-bottom: 16px;
	}
	.title {
		font-size: 18px;
		font-weight: 500;
	}
	.actions {
		display: flex;
		gap: var(--size-8);
	}
	.conflict-box-inner {
		display: flex;
		flex-direction: column;
		background-color: bisque;
		padding: 32px;
		border-radius: 10px;
		gap: 12px;
	}
	.conflict-box {
		position: relative;
		padding: 100px;
	}
</style>
