<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileList from '$components/v3/FileList.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import type { Commit } from '$lib/branches/v3';

	interface Props {
		projectId: string;
		commit: Commit;
	}

	const { projectId, commit }: Props = $props();
	const [stackService] = inject(StackService);
	const changesResult = $derived(stackService.commitChanges(projectId, commit.id));
</script>

<div class="changed-files">
	<ReduxResult result={changesResult.current}>
		{#snippet children(changes)}
			<div class="header text-13 text-bold">
				<span>Changed files</span>
				<Badge>{changes.length}</Badge>
			</div>
			{#if changes.length > 0}
				<FileList {projectId} {changes} />
			{:else}
				<div class="text-12 text-body helper-text">(no changed files)</div>
			{/if}
		{/snippet}
	</ReduxResult>
</div>

<style>
	.changed-files {
		display: flex;
		flex-direction: column;
		border-radius: var(--radius-l);
		border: 1px solid var(--clr-border-2);
		overflow: hidden;
	}

	.header {
		padding: 14px 14px 16px 14px;
		display: flex;
		align-items: center;
		gap: 4px;
	}
</style>
