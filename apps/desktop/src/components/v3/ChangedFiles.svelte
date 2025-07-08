<script lang="ts">
	import Drawer from '$components/v3/Drawer.svelte';
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
	import type { Snippet } from 'svelte';

	type Props = {
		projectId: string;
		stackId?: string;
		selectionId: SelectionId;
		changes: TreeChange[];
		title: string;
		active?: boolean;
		conflictEntries?: ConflictEntriesObj;
		draggableFiles?: boolean;
		collapsible?: boolean;
		grow?: boolean;
		ontoggle?: () => void;
		resizer?: Snippet<[{ element: HTMLDivElement; collapsed?: boolean }]>;
	};

	const {
		projectId,
		stackId,
		selectionId,
		changes,
		title,
		active,
		conflictEntries,
		draggableFiles,
		collapsible,
		grow,
		resizer
	}: Props = $props();

	const [intelligentScrollingService] = inject(IntelligentScrollingService);

	let listMode: 'list' | 'tree' = $state('tree');
</script>

<Drawer
	{collapsible}
	{resizer}
	{grow}
	transparent
	headerNoPaddingLeft={collapsible}
	bottomBorder={!!resizer || !collapsible}
>
	{#snippet header()}
		<h4 class="text-14 text-semibold truncate">{title}</h4>
		<Badge>{changes.length}</Badge>
	{/snippet}
	{#snippet extraActions()}
		<FileListMode bind:mode={listMode} persist="committed" />
	{/snippet}
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
</Drawer>

<style lang="postcss">
</style>
