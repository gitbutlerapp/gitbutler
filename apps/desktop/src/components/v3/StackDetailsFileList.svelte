<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileList from '$components/v3/FileList.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import type { Commit } from '$lib/branches/v3';

	interface Props {
		projectId: string;
		commit: Commit;
	}

	const { projectId, commit }: Props = $props();
	const [stackService] = inject(StackService);
	const changesQuery = $derived(stackService.commitChanges(projectId, commit.id).current);
</script>

<div class="wrapper">
	<div class="header text-13 text-bold">Changed files</div>
	{#if changesQuery}
		<ReduxResult result={changesQuery}>
			{#snippet children(changes)}
				{#if changes.length > 0}
					<FileList {projectId} {changes} />
				{:else}
					<div class="text-12 text-body helper-text">
						<div>You're all caught up!</div>
						<div>No files need committing</div>
					</div>
				{/if}
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
