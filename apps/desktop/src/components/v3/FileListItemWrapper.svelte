<!-- This is a V3 replacement for `FileListItemWrapper.svelte` -->
<script lang="ts">
	import FileContextMenu from '$components/v3/FileContextMenu.svelte';
	import { BranchStack } from '$lib/branches/branch';
	import { draggableChips } from '$lib/dragging/draggable';
	import { ChangeDropData } from '$lib/dragging/draggables';
	import { getFilename } from '$lib/files/utils';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { key } from '$lib/selection/key';
	import { computeChangeStatus } from '$lib/utils/fileStatus';
	import { getContext, maybeGetContextStore } from '@gitbutler/shared/context';
	import FileListItem from '@gitbutler/ui/file/FileListItem.svelte';
	import type { TreeChange } from '$lib/hunks/change';

	interface Props {
		change: TreeChange;
		commitId?: string;
		projectId: string;
		selected: boolean;
		showCheckbox?: boolean;
		onclick: (e: MouseEvent) => void;
		onkeydown?: (e: KeyboardEvent) => void;
	}

	const {
		change: change,
		commitId,
		projectId,
		selected,
		showCheckbox,
		onclick,
		onkeydown
	}: Props = $props();

	const stack = maybeGetContextStore(BranchStack);
	const stackId = $derived($stack?.id);
	const idSelection = getContext(IdSelection);

	let contextMenu = $state<ReturnType<typeof FileContextMenu>>();
	let draggableEl: HTMLDivElement | undefined = $state();
	let indeterminate = $state(false);
	let checked = $state(false);
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
		id={key(change.path, commitId)}
		filePath={change.path}
		fileStatus={computeChangeStatus(change)}
		{selected}
		{showCheckbox}
		{checked}
		{indeterminate}
		draggable={true}
		{onclick}
		{onkeydown}
		locked={false}
		conflicted={false}
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

<style lang="postcss">
	/* blah */
</style>
