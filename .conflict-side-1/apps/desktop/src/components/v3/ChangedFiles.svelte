<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileList from '$components/v3/FileList.svelte';
	import FileListMode from '$components/v3/FileListMode.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import { stickyHeader } from '@gitbutler/ui/utils/stickyHeader';

	type Props = {
		projectId: string;
		stackId: string;
		selectionId:
			| {
					type: 'commit';
					commitId: string;
			  }
			| {
					type: 'branch';
					stackId: string;
					branchName: string;
			  };
	};

	const { projectId, stackId, selectionId }: Props = $props();
	const [stackService] = inject(StackService);
	const changesResult = $derived(
		selectionId.type === 'commit'
			? stackService.commitChanges(projectId, selectionId.commitId)
			: stackService.branchChanges(projectId, selectionId.stackId, selectionId.branchName)
	);

	const headerTitle = $derived.by(() => {
		switch (selectionId.type) {
			case 'commit':
				return 'Changed files';
			case 'branch':
				return 'All changed files';
		}
	});

	let listMode: 'list' | 'tree' = $state('tree');
</script>

{#if changesResult}
	<div class="changed-files">
		<ReduxResult {stackId} {projectId} result={changesResult.current}>
			{#snippet children(changes, { stackId, projectId })}
				<div class="changed-files__header" use:stickyHeader>
					<div class="changed-files__header-left">
						<h4 class="text-14 text-semibold">{headerTitle}</h4>
						<Badge>{changes.length}</Badge>
					</div>
					<FileListMode bind:mode={listMode} persist="committed" />
				</div>
				{#if changes.length > 0}
					<FileList {projectId} {stackId} {changes} {listMode} {selectionId} />
				{:else}
					<div class="text-12 text-body helper-text">(no changed files)</div>
				{/if}
			{/snippet}
		</ReduxResult>
	</div>
{:else}
	<p class="text-13 text-bold">Malformed props</p>
{/if}

<style>
	.changed-files {
		position: relative;
		display: flex;
		flex-direction: column;
	}

	.changed-files__header {
		padding: 10px 10px 10px 14px;
		display: flex;
		align-items: center;
		gap: 4px;
		justify-content: space-between;
		background-color: var(--clr-bg-1);
	}

	.changed-files__header-left {
		display: flex;
		align-items: center;
		gap: 4px;
	}
</style>
