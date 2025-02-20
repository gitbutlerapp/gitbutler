<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileList from '$components/v3/FileList.svelte';
	import { ProjectService } from '$lib/project/projectService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import type { Commit } from '$lib/branches/v3';

	interface Props {
		commit: Commit;
	}

	const { commit }: Props = $props();

	const [projectService, stackService] = inject(ProjectService, StackService);
	const projectId = projectService.projectId;

	const commitChangesQuery = $derived(
		commit?.id ? stackService.getCommitChanges(projectId, commit?.id) : undefined
	);
</script>

<div class="wrapper">
	<div class="header text-13 text-bold">Changed files</div>
	{#if commitChangesQuery}
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
	{/if}
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
