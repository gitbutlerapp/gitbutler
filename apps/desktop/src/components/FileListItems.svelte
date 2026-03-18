<!--
	Compound component that renders the file list (list or tree mode).
	Must be a child of <FileListProvider>.

	Usage:
	```svelte
	<FileListProvider {changes} {selectionId}>
		<FileListItems mode="list" draggable />
	</FileListProvider>
	```
-->
<script lang="ts">
	import FileListItemWrapper from "$components/FileListItemWrapper.svelte";
	import FileTreeNode from "$components/FileTreeNode.svelte";
	import LazyList from "$components/LazyList.svelte";
	import {
		getLockedCommitIds,
		getLockedTargets,
		isFileLocked,
	} from "$lib/dependencies/dependencies";
	import { DEPENDENCY_SERVICE } from "$lib/dependencies/dependencyService.svelte";
	import { abbreviateFolders, changesToFileTree } from "$lib/files/filetreeV3";
	import { isExecutableStatus } from "$lib/hunks/change";
	import {
		getFileListContext,
		type FileListKeyHandler,
	} from "$lib/selection/fileListController.svelte";
	import { inject } from "@gitbutler/core/context";
	import { focusable } from "@gitbutler/ui/focus/focusable";
	import type { ConflictEntriesObj } from "$lib/files/conflicts";
	import type { TreeChange } from "$lib/hunks/change";

	type Props = {
		projectId: string;
		stackId?: string;
		mode: "list" | "tree";
		showCheckboxes?: boolean;
		draggable?: boolean;
		showLockedIndicator?: boolean;
		visibleRange?: { start: number; end: number };
		/** nick → file paths mapping from IRC working files broadcast */
		ircWorkingFiles?: Record<string, string[]>;
		/** Per-file conflict hints (rendered inline on each item) */
		conflictEntries?: ConflictEntriesObj;
		dataTestId?: string;
		/** Called when a file is selected (click, Enter/Space/l, or arrow navigation). */
		onselect?: (change: TreeChange, index: number) => void;
		/** Extra keyboard handlers injected by the consumer (e.g. AI shortcuts). */
		extraKeyHandlers?: FileListKeyHandler[];
	};

	const {
		projectId,
		stackId,
		mode,
		showCheckboxes,
		draggable,
		showLockedIndicator = false,
		visibleRange,
		ircWorkingFiles,
		conflictEntries,
		dataTestId,
		onselect,
		extraKeyHandlers,
	}: Props = $props();

	const controller = getFileListContext();
	const dependencyService = inject(DEPENDENCY_SERVICE);

	/** Invert nick→paths map to path→nicks for per-file lookup. */
	const ircWorkingUsersByPath = $derived.by(() => {
		if (!ircWorkingFiles) return undefined;
		const map = new Map<string, string[]>();
		for (const [nick, paths] of Object.entries(ircWorkingFiles)) {
			for (const p of paths) {
				const nicks = map.get(p);
				if (nicks) {
					nicks.push(nick);
				} else {
					map.set(p, [nick]);
				}
			}
		}
		return map;
	});

	const filePaths = $derived(controller.changes.map((change) => change.path));
	const fileDependenciesQuery = $derived(
		showLockedIndicator ? dependencyService.filesDependencies(projectId, filePaths) : null,
	);
	const fileDependencies = $derived(fileDependenciesQuery?.result.data || []);
</script>

{#snippet fileTemplate(change: TreeChange, idx: number, depth: number = 0, isLast: boolean = false)}
	{@const isExecutable = isExecutableStatus(change.status)}
	{@const selected = controller.isSelected(change.path)}
	{@const locked = showLockedIndicator && isFileLocked(change.path, fileDependencies)}
	{@const lockedCommitIds = showLockedIndicator
		? getLockedCommitIds(change.path, fileDependencies)
		: []}
	{@const lockedTargets = showLockedIndicator
		? getLockedTargets(change.path, fileDependencies)
		: []}
	<FileListItemWrapper
		selectionId={controller.selectionId}
		{change}
		{projectId}
		{stackId}
		{selected}
		listMode={mode}
		{depth}
		active={controller.active}
		{locked}
		{lockedCommitIds}
		{lockedTargets}
		{isLast}
		notched={controller.hasSelectionInList &&
			visibleRange !== undefined &&
			idx >= visibleRange.start &&
			idx < visibleRange.end}
		{draggable}
		executable={isExecutable}
		showCheckbox={showCheckboxes}
		ircWorkingUsers={ircWorkingUsersByPath?.get(change.path)}
		focusableOpts={{
			onKeydown: (e) => {
				// 1. Activation keys (Enter/Space/l)
				if (controller.handleActivation(change, idx, e)) {
					onselect?.(change, idx);
					return true;
				}
				// 2. Extra handlers (e.g. AI shortcuts)
				if (extraKeyHandlers) {
					for (const handler of extraKeyHandlers) {
						if (handler(change, idx, e)) return true;
					}
				}
				// 3. Arrow/vim navigation — only claim the event if we moved
				const navigatedIndex = controller.handleNavigation(e);
				if (navigatedIndex !== undefined && navigatedIndex !== idx) {
					const navigatedChange = controller.changes[navigatedIndex];
					if (navigatedChange) {
						onselect?.(navigatedChange, navigatedIndex);
					}
					return true;
				}
			},
			focusable: true,
		}}
		onclick={(e) => {
			e.stopPropagation();
			controller.select(e, change, idx);
			if (controller.isSelected(change.path)) {
				onselect?.(change, idx);
			}
		}}
		{conflictEntries}
	/>
{/snippet}

<div
	data-testid={dataTestId}
	class="file-list"
	use:focusable={{
		vertical: true,
		onActive: (value) => (controller.active = value),
	}}
>
	{#if controller.changes.length > 0}
		{#if mode === "tree"}
			{@const node = abbreviateFolders(changesToFileTree(controller.changes))}
			<FileTreeNode
				isRoot
				{projectId}
				selectionId={controller.selectionId}
				{stackId}
				{node}
				{showCheckboxes}
				draggableFiles={draggable}
				changes={controller.changes}
				{fileTemplate}
			/>
		{:else}
			<LazyList items={controller.changes} chunkSize={100}>
				{#snippet template(change, context)}
					<!--
						There is a bug here related to the reactivity of `idSelection.has`,
						affecting somehow the first item in the list of files.. but only where
						used for the "assigned files" of the workspace.

						This unused variable is a workaround, while present the reactivity
						works as expected.

						TODO: Bisect this issue, it was introduced between nightly version
						0.5.1705 and 0.5.1783.
						-->
					{@const _selected = controller.isSelected(change.path)}
					{@render fileTemplate(change, context.index, 0, context.last)}
				{/snippet}
			</LazyList>
		{/if}
	{/if}
</div>

<style lang="postcss">
	.file-list {
		display: flex;
		flex-direction: column;
	}
</style>
