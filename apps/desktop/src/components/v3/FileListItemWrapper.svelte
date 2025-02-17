<!-- This is a V3 replacement for `FileListItemWrapper.svelte` -->
<script lang="ts">
	import FileContextMenu from '$components/v3/FileContextMenu.svelte';
	import { BranchStack } from '$lib/branches/branch';
	import { draggableChips } from '$lib/dragging/draggable';
	import { ChangeDropData } from '$lib/dragging/draggables';
	import { getFilename } from '$lib/files/utils';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { key } from '$lib/selection/key';
	import { computeChangeStatus } from '$lib/utils/fileStatus';
	import { getContext, maybeGetContextStore } from '@gitbutler/shared/context';
	import FileListItem from '@gitbutler/ui/file/FileListItem.svelte';
	import type { TreeChange } from '$lib/hunks/change';
	import type { Snippet } from 'svelte';

	interface Props {
		change: TreeChange;
		commitId?: string;
		projectId: string;
		selected: boolean;
		showCheckbox?: boolean;
		onclick: (e: MouseEvent) => void;
		onkeydown?: (e: KeyboardEvent) => void;
		children: Snippet;
	}

	const {
		change: change,
		commitId,
		projectId,
		selected,
		showCheckbox,
		onclick,
		onkeydown,
		children
	}: Props = $props();

	const stack = maybeGetContextStore(BranchStack);
	const stackId = $derived($stack?.id);
	const idSelection = getContext(IdSelection);
	const changeSelection = getContext(ChangeSelectionService);

	let contextMenu = $state<ReturnType<typeof FileContextMenu>>();
	let draggableEl: HTMLDivElement | undefined = $state();
	let open = $state(false);

	const selection = $derived(changeSelection.getById(change.path).current);
	let indeterminate = $derived(selection && selection.type === 'partial');

	function onCheck() {
		if (selection) {
			changeSelection.remove(change.path);
		} else {
			const { path, pathBytes, previousPathBytes } = change;
			changeSelection.add({
				type: 'full',
				path,
				pathBytes,
				previousPathBytes
			});
		}
	}
</script>

<div
	bind:this={draggableEl}
	use:draggableChips={{
		label: getFilename(change.path),
		filePath: change.path,
		data: new ChangeDropData(stackId || '', change, idSelection, commitId),
		viewportId: 'board-viewport',
		selector: '.selected-draggable'
	}}
>
	<FileContextMenu
		bind:this={contextMenu}
		trigger={draggableEl}
		isUnapplied={false}
		branchId={$stack?.id}
		isBinary={false}
	/>

	<FileListItem
		bind:open
		id={key(change.path, commitId)}
		filePath={change.path}
		fileStatus={computeChangeStatus(change)}
		{selected}
		{showCheckbox}
		checked={!!selection}
		{indeterminate}
		draggable={true}
		{onkeydown}
		locked={false}
		conflicted={false}
		{onclick}
		oncheck={onCheck}
		oncontextmenu={(e) => {
			const changes = idSelection.treeChanges(projectId);
			if (idSelection.has(change.path, commitId)) {
				contextMenu?.open(e, { files: changes });
			} else {
				contextMenu?.open(e, { files: [change] });
			}
		}}
	/>
</div>
{#if open}
	<div class="diff">
		{@render children()}
	</div>
{/if}

<style lang="postcss">
	.diff {
	}
</style>
