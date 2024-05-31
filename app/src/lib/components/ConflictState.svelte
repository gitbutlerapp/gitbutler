<script lang="ts" async="true">
	import { Project } from '$lib/backend/projects';
	import { getContext } from '$lib/utils/context';
	import { listRemoteCommitFiles } from '$lib/vbranches/remoteCommits';
	import { BranchController } from '$lib/vbranches/branchController';
	import Tag from './Tag.svelte';

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
	<h1>You Are Now in Conflict Resolution Mode</h1>
	<div>Conflict Commit : <code>{commit}</code></div>
	<div>Branch: <code>{branch.id}</code></div>
	<div class="title">Files:</div>
	{#each files as file}
		<div>{file.path}</div>
	{/each}
	<div class="title">Actions:</div>
	<div class="actions">
		<Tag style="success" on:click={resolveConflictFinish}>Resolve</Tag>
		<Tag style="error" on:click={resolveConflictAbandon}>Abandon</Tag>
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
	.conflict-box {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: var(--size-8);
		padding: 100px;
	}
	.actions {
		display: flex;
		gap: var(--size-8);
	}
</style>
