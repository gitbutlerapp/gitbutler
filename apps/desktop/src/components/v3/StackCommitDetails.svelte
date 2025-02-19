<script lang="ts">
	import StackDetailsCommitHeader from './StackDetailsCommitHeader.svelte';
	import StackDetailsFileList from './StackDetailsFileList.svelte';
	import { Commit } from '$lib/commits/commit';
	import { CommitService } from '$lib/commits/commitService.svelte';
	import { ProjectService } from '$lib/project/projectService';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { getContext } from 'svelte';

	interface Props {
		selectedCommitId: string | undefined;
	}

	let { selectedCommitId = $bindable() }: Props = $props();

	const projectService = getContext<ProjectService>(ProjectService);
	const projectId = projectService.projectId;
	const commitService = getContext<CommitService>(CommitService);
	let commit = $state<Commit>();

	async function getCommitData() {
		if (selectedCommitId) {
			commit = await commitService.find(projectId, selectedCommitId);
		}
	}

	$effect(() => {
		getCommitData();
	});
</script>

<div class="wrapper">
	<div>
		<button type="button" class="exit-btn" onclick={() => (selectedCommitId = undefined)}>
			<Icon name="cross" />
		</button>
		{#if commit}
			<StackDetailsCommitHeader {commit} />
		{/if}
	</div>
	<div class="body">
		{#if commit}
			<StackDetailsFileList {commit} />
		{/if}
	</div>
</div>

<style>
	.wrapper {
		position: relative;
		flex: 1;
		display: flex;
		flex-direction: column;

		background-color: var(--clr-bg-1);
	}

	.exit-btn {
		position: absolute;
		top: 8px;
		right: 8px;
	}

	.body {
		display: flex;
		flex-direction: column;
	}
</style>
