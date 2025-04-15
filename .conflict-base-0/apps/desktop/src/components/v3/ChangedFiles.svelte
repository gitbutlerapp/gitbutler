<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileList from '$components/v3/FileList.svelte';
	import FileListMode from '$components/v3/FileListMode.svelte';
	import emptyFolderSvg from '$lib/assets/empty-state/empty-folder.svg?raw';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import EmptyStatePlaceholder from '@gitbutler/ui/EmptyStatePlaceholder.svelte';
	import { stickyHeader } from '@gitbutler/ui/utils/stickyHeader';
	import type { SelectionId } from '$lib/selection/key';

	type Props = {
		projectId: string;
		stackId: string;
		selectionId: SelectionId;
	};

	const { projectId, stackId, selectionId }: Props = $props();
	const [stackService] = inject(StackService);
	const changesResult = $derived(
		selectionId.type === 'commit'
			? stackService.commitChanges(projectId, selectionId.commitId)
			: selectionId.type === 'branch'
				? stackService.branchChanges(projectId, selectionId.stackId, selectionId.branchName)
				: undefined
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
					<EmptyStatePlaceholder image={emptyFolderSvg} width={180} gap={4}>
						{#snippet caption()}
							No files changed
						{/snippet}
					</EmptyStatePlaceholder>
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
		min-height: 100%;
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
		gap: 6px;
	}
</style>
