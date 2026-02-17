<script lang="ts">
	import ChangedFilesContextMenu from "$components/ChangedFilesContextMenu.svelte";
	import { draggableChips } from "$lib/dragging/draggable";
	import { FileChangeDropData } from "$lib/dragging/draggables";
	import { DROPZONE_REGISTRY } from "$lib/dragging/registry";
	import { getFilename } from "$lib/files/utils";
	import { type TreeChange } from "$lib/hunks/change";
	import { FILE_SELECTION_MANAGER } from "$lib/selection/fileSelectionManager.svelte";
	import { type SelectionId } from "$lib/selection/key";
	import { SETTINGS } from "$lib/settings/userSettings";
	import { computeChangeStatus } from "$lib/utils/fileStatus";
	import { inject } from "@gitbutler/core/context";
	import { FileViewHeader, TestId } from "@gitbutler/ui";
	import { DRAG_STATE_SERVICE } from "@gitbutler/ui/drag/dragStateService.svelte";
	import { sticky as stickyAction } from "@gitbutler/ui/utils/sticky";
	import type { UnifiedDiff } from "$lib/hunks/diff";

	interface Props {
		projectId: string;
		stackId?: string;
		change: TreeChange;
		diff?: UnifiedDiff | null;
		selectionId: SelectionId;
		draggable?: boolean;
		executable?: boolean;
		scrollContainer?: HTMLDivElement;
		onCloseClick?: () => void;
	}

	const {
		change,
		diff,
		selectionId,
		projectId,
		stackId,
		draggable,
		executable,
		scrollContainer,
		onCloseClick,
	}: Props = $props();

	const idSelection = inject(FILE_SELECTION_MANAGER);
	const dropzoneRegistry = inject(DROPZONE_REGISTRY);
	const dragStateService = inject(DRAG_STATE_SERVICE);
	const userSettings = inject(SETTINGS);

	let contextMenu = $state<ReturnType<typeof ChangedFilesContextMenu>>();
	let draggableEl: HTMLDivElement | undefined = $state();
	let isStuck = $state(false);

	const previousTooltipText = $derived(
		change.status.type === "Rename" && change.status.subject.previousPath
			? `${change.status.subject.previousPath} →\n${change.path}`
			: undefined,
	);

	const lineChangesStat = $derived.by(() => {
		if (diff && diff.type === "Patch") {
			return {
				added: diff.subject.linesAdded,
				removed: diff.subject.linesRemoved,
			};
		}
		return undefined;
	});

	async function onContextMenu(e: MouseEvent) {
		const changes = await idSelection.treeChanges(projectId, selectionId);
		if (idSelection.has(change.path, selectionId) && changes.length > 0) {
			contextMenu?.open(e, { changes });
			return;
		}
		contextMenu?.open(e, { changes: [change] });
	}
</script>

<div
	data-testid={TestId.FileListItem}
	class="fileviewheader-wrapper"
	data-remove-from-panning
	class:stuck={isStuck}
	bind:this={draggableEl}
	use:draggableChips={{
		label: getFilename(change.path),
		filePath: change.path,
		data: new FileChangeDropData(projectId, change, idSelection, selectionId, stackId || undefined),
		disabled: !draggable,
		chipType: "file",
		dropzoneRegistry,
		dragStateService,
	}}
	use:stickyAction={{
		enabled: true,
		scrollContainer,
		onStuck: (stuck) => {
			isStuck = stuck;
		},
	}}
>
	<ChangedFilesContextMenu
		bind:this={contextMenu}
		{projectId}
		{stackId}
		trigger={draggableEl}
		{selectionId}
	/>

	<FileViewHeader
		filePath={change.path}
		fileStatus={computeChangeStatus(change)}
		{draggable}
		linesAdded={lineChangesStat?.added}
		linesRemoved={lineChangesStat?.removed}
		fileStatusTooltip={previousTooltipText}
		{executable}
		pathFirst={$userSettings.pathFirst}
		oncontextmenu={(e) => {
			e.stopPropagation();
			e.preventDefault();
			onContextMenu(e);
		}}
		oncloseclick={onCloseClick}
	/>
</div>

<style lang="postcss">
	.fileviewheader-wrapper {
		display: block;
		z-index: var(--z-lifted);

		&.stuck {
			border-bottom: 1px solid var(--clr-border-2);
			background-color: var(--clr-bg-1);
			box-shadow: 0 1px 8px rgba(0, 0, 0, 0.1);
		}
	}
</style>
