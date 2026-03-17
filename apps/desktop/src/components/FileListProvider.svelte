<!--
	Context provider for file list compound components.

	Creates a FileListController and sets it into Svelte context so that
	children like <FileListItems> and <FileListConflicts> can access
	shared selection state, keyboard handling, and focus management.

	Usage:
	```svelte
	<FileListProvider {changes} {selectionId}>
		<FileListItems mode="list" />
	</FileListProvider>
	```
-->
<script lang="ts">
	import { FileListController, setFileListContext } from "$lib/selection/fileListController.svelte";
	import type { TreeChange } from "$lib/hunks/change";
	import type { SelectionId } from "$lib/selection/key";
	import type { Snippet } from "svelte";

	type Props = {
		changes: TreeChange[];
		selectionId: SelectionId;
		allowUnselect?: boolean;
		children: Snippet;
	};

	const { changes, selectionId, allowUnselect = true, children }: Props = $props();

	const controller = new FileListController({
		changes: () => changes,
		selectionId: () => selectionId,
		allowUnselect: () => allowUnselect,
	});

	setFileListContext(controller);
</script>

{@render children()}
