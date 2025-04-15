<script lang="ts">
	import FileList from '$components/v3/FileList.svelte';
	import FileListMode from '$components/v3/FileListMode.svelte';
	import emptyFolderSvg from '$lib/assets/empty-state/empty-folder.svg?raw';
	import { Focusable, FocusManager } from '$lib/focus/focusManager.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import EmptyStatePlaceholder from '@gitbutler/ui/EmptyStatePlaceholder.svelte';
	import { stickyHeader } from '@gitbutler/ui/utils/stickyHeader';
	import type { TreeChange } from '$lib/hunks/change';
	import type { SelectionId } from '$lib/selection/key';

	type Props = {
		projectId: string;
		stackId?: string;
		selectionId: SelectionId;
		changes: TreeChange[];
		title: string;
	};

	const { projectId, stackId, selectionId, changes, title }: Props = $props();

	const [focusManager] = inject(FocusManager);

	let focusGroup = focusManager.radioGroup({
		triggers: [Focusable.UncommittedChanges, Focusable.ChangedFiles]
	});
	const listActive = $derived(focusGroup.current === Focusable.ChangedFiles);

	let listMode: 'list' | 'tree' = $state('tree');
</script>

<div
	class="changed-files"
	use:focusable={{ id: Focusable.ChangedFiles, parentId: Focusable.Workspace }}
>
	<div class="changed-files__header" use:stickyHeader>
		<div class="changed-files__header-left">
			<h4 class="text-14 text-semibold">{title}</h4>
			<Badge>{changes.length}</Badge>
		</div>
		<FileListMode bind:mode={listMode} persist="committed" />
	</div>
	{#if changes.length > 0}
		<FileList {selectionId} {projectId} {stackId} {changes} {listMode} {listActive} />
	{:else}
		<EmptyStatePlaceholder image={emptyFolderSvg} width={180} gap={4}>
			{#snippet caption()}
				No files changed
			{/snippet}
		</EmptyStatePlaceholder>
	{/if}
</div>

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
