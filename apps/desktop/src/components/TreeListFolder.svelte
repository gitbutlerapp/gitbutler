<script lang="ts">
	import ChangedFilesContextMenu from "$components/ChangedFilesContextMenu.svelte";
	import { draggableChips } from "$lib/dragging/draggable";
	import { FolderChangeDropData } from "$lib/dragging/draggables";
	import { DROPZONE_REGISTRY } from "$lib/dragging/registry";
	import { getAllChanges, nodePath, type TreeNode } from "$lib/files/filetreeV3";
	import { FILE_SELECTION_MANAGER } from "$lib/selection/fileSelectionManager.svelte";
	import { UNCOMMITTED_SERVICE } from "$lib/selection/uncommittedService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { FolderListItem } from "@gitbutler/ui";
	import { DRAG_STATE_SERVICE } from "@gitbutler/ui/drag/dragStateService.svelte";
	import type { SelectionId } from "$lib/selection/key";
	import type { FocusableOptions } from "@gitbutler/ui/focus/focusManager";

	type Props = {
		projectId: string;
		stackId?: string;
		selectionId: SelectionId;
		node: TreeNode & { kind: "dir" };
		depth: number;
		showCheckbox?: boolean;
		draggable?: boolean;
		isExpanded?: boolean;
		active?: boolean;
		focusableOpts?: FocusableOptions;
		onclick?: (e: MouseEvent) => void;
		onmousedown?: (e: MouseEvent) => void;
		ontoggle?: (expanded: boolean) => void;
		testId?: string;
	};

	const {
		projectId,
		stackId,
		selectionId,
		node,
		depth,
		showCheckbox,
		draggable,
		isExpanded,
		active,
		focusableOpts,
		onclick,
		onmousedown,
		ontoggle,
		testId,
	}: Props = $props();

	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const dropzoneRegistry = inject(DROPZONE_REGISTRY);
	const dragStateService = inject(DRAG_STATE_SERVICE);
	const idSelection = inject(FILE_SELECTION_MANAGER);

	const folderPath = $derived(nodePath(node));
	const selectionStatus = $derived(uncommittedService.folderCheckStatus(stackId, folderPath));

	const folderSelected = $derived.by(() => {
		const folderChanges = getAllChanges(node);
		if (folderChanges.length === 0) return false;
		return folderChanges.every((c) => idSelection.has(c.path, selectionId));
	});

	let contextMenu: ReturnType<typeof ChangedFilesContextMenu>;
	let draggableEl: HTMLDivElement | undefined = $state();

	function handleCheck(checked: boolean) {
		if (checked) {
			uncommittedService.checkDir(stackId || null, folderPath);
		} else {
			uncommittedService.uncheckDir(stackId || null, folderPath);
		}
	}

	function getTreeChanges() {
		return getAllChanges(node);
	}

	function onContextMenu(e: MouseEvent) {
		const item = {
			path: folderPath,
			changes: getTreeChanges(),
		};
		contextMenu?.open(e, item);
	}

	const draggableDisabled = $derived(!draggable || showCheckbox);
</script>

<div
	class="folder-list-item-wrapper"
	data-remove-from-panning
	bind:this={draggableEl}
	use:draggableChips={{
		label: node.name,
		filePath: folderPath,
		data: new FolderChangeDropData(folderPath, getTreeChanges, selectionId, stackId),
		disabled: draggableDisabled,
		chipType: "folder",
		dropzoneRegistry,
		dragStateService,
	}}
>
	<ChangedFilesContextMenu
		bind:this={contextMenu}
		{projectId}
		{stackId}
		trigger={draggableEl}
		{selectionId}
	/>

	<FolderListItem
		{testId}
		name={node.name}
		{depth}
		{isExpanded}
		{showCheckbox}
		checked={selectionStatus.current === "checked"}
		indeterminate={selectionStatus.current === "indeterminate"}
		draggable={!draggableDisabled}
		selected={folderSelected}
		{active}
		actionOpts={focusableOpts}
		oncheck={(e) => handleCheck(e.currentTarget.checked)}
		{onclick}
		{onmousedown}
		{ontoggle}
		oncontextmenu={onContextMenu}
	/>
</div>
