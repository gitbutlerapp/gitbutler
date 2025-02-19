<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileList from '$components/v3/FileList.svelte';
	import { Commit } from '$lib/commits/commit';
	import { ProjectService } from '$lib/project/projectService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext } from 'svelte';

	interface Props {
		commit: Commit;
	}

	const { commit }: Props = $props();

	const projectService = getContext<ProjectService>(ProjectService);
	const projectId = projectService.projectId;

	const stackService = getContext<StackService>(StackService);

	const commitChangesQuery = $derived(
		stackService.commitChanges(projectId, commit?.parentIds[0], commit?.id)
	);
</script>

<div class="wrapper">
	<div class="header text-13 text-bold">Changed files</div>
	<ReduxResult result={commitChangesQuery.current}>
		{#snippet children(changes)}
			<FileList {projectId} {changes} />
		{/snippet}
		{#snippet empty()}
			<div class="text-12 text-body helper-text">
				<div>You're all caught up!</div>
				<div>No files need committing</div>
			</div>
		{/snippet}
	</ReduxResult>
</div>

<style>
	.wrapper {
		display: flex;
		flex-direction: column;
	}

	.header {
		padding: 14px 14px 16px 14px;
	}
</style>
