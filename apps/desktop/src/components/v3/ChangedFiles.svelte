<script lang="ts">
	import FileList from '$components/v3/FileList.svelte';
	import FileListMode from '$components/v3/FileListMode.svelte';
	import emptyFolderSvg from '$lib/assets/empty-state/empty-folder.svg?raw';
	import { IntelligentScrollingService } from '$lib/intelligentScrolling/service';
	import { inject } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import EmptyStatePlaceholder from '@gitbutler/ui/EmptyStatePlaceholder.svelte';
	import type { ConflictEntriesObj } from '$lib/files/conflicts';
	import type { TreeChange } from '$lib/hunks/change';
	import type { SelectionId } from '$lib/selection/key';

	type Props = {
		projectId: string;
		stackId?: string;
		selectionId: SelectionId;
		changes: TreeChange[];
		title: string;
		active?: boolean;
		conflictEntries?: ConflictEntriesObj;
		draggableFiles?: boolean;
	};

	const {
		projectId,
		stackId,
		selectionId,
		changes,
		title,
		active,
		conflictEntries,
		draggableFiles
	}: Props = $props();

	const [intelligentScrollingService] = inject(IntelligentScrollingService);

	let listMode: 'list' | 'tree' = $state('tree');
</script>

<div class="changed-files__header">
	<div class="changed-files__header-left">
		<h4 class="text-14 text-semibold truncate">{title}</h4>
		<Badge>{changes.length}</Badge>
	</div>
	<FileListMode bind:mode={listMode} persist="committed" />
</div>
{#if changes.length > 0}
	<FileList
		{selectionId}
		{projectId}
		{stackId}
		{changes}
		{listMode}
		{active}
		{conflictEntries}
		{draggableFiles}
		onselect={() => {
			if (stackId) {
				intelligentScrollingService.show(projectId, stackId, 'diff');
			}
		}}
	/>
{:else}
	<EmptyStatePlaceholder image={emptyFolderSvg} width={180} gap={4}>
		{#snippet caption()}
			No files changed
		{/snippet}
	</EmptyStatePlaceholder>
{/if}

<style lang="postcss">
	.changed-files__header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 10px 10px 10px 14px;
		gap: 8px;
		border-top: 1px solid var(--clr-border-2);
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);
	}

	.changed-files__header-left {
		display: flex;
		align-items: center;
		overflow: hidden;
		gap: 6px;
	}
</style>
