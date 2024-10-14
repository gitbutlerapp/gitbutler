<script lang="ts">
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { editor } from '$lib/editorLink/editorLink';
	import { openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { getContext } from '@gitbutler/shared/context';

	interface Props {
		target: HTMLElement | undefined;
		filePath: string;
		projectPath: string | undefined;
		readonly: boolean;
	}

	let { target, filePath, projectPath, readonly }: Props = $props();

	const branchController = getContext(BranchController);

	let contextMenu: ReturnType<typeof ContextMenu>;

	export function open(e: MouseEvent, item: any) {
		contextMenu.open(e, item);
	}

	export function close() {
		contextMenu.close();
	}
</script>

<ContextMenu bind:this={contextMenu} {target} openByMouse>
	{#snippet children(item)}
		<ContextMenuSection>
			{#if item.hunk !== undefined && !readonly}
				<ContextMenuItem
					label="Discard"
					on:click={() => {
						branchController.unapplyHunk(item.hunk);
						contextMenu.close();
					}}
				/>
			{/if}
			{#if item.lineNumber}
				<ContextMenuItem
					label="Open in VSCode"
					on:click={() => {
						projectPath &&
							openExternalUrl(`${$editor}://file${projectPath}/${filePath}:${item.lineNumber}`);
						contextMenu.close();
					}}
				/>
			{/if}
		</ContextMenuSection>
	{/snippet}
</ContextMenu>
