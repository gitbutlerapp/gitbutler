<script lang="ts">
	import { ContextMenu, ContextMenuItem, ContextMenuSection } from '@gitbutler/ui';
	import type { RuleFilterType } from '$lib/rules/rule';

	type Props = {
		addedFilterTypes: RuleFilterType[];
		addFromFilter: (type: RuleFilterType) => void;
		addEmpty?: () => void;
		trigger?: HTMLElement;
	};

	const { addFromFilter, trigger, addEmpty, addedFilterTypes }: Props = $props();

	let contextMenu = $state<ContextMenu>();

	function filterHasBeenAdded(type: RuleFilterType): boolean {
		return addedFilterTypes.includes(type);
	}

	function handleAddFilter(type: RuleFilterType) {
		addFromFilter(type);
		contextMenu?.close();
	}

	function handleAddEmpty() {
		addEmpty?.();
		contextMenu?.close();
	}

	export function toggle(e: MouseEvent) {
		contextMenu?.toggle(e);
	}
</script>

<ContextMenu bind:this={contextMenu} leftClickTrigger={trigger}>
	<ContextMenuSection>
		<ContextMenuItem
			icon="folder"
			label="File or folder path"
			disabled={filterHasBeenAdded('pathMatchesRegex')}
			onclick={() => {
				handleAddFilter('pathMatchesRegex');
			}}
		/>
		<ContextMenuItem
			icon="text-width"
			label="Contains text (coming soon)"
			disabled={filterHasBeenAdded('contentMatchesRegex') || true}
			onclick={() => {
				handleAddFilter('contentMatchesRegex');
			}}
		/>
		<ContextMenuItem
			icon="file-changes"
			label="Change type (coming soon)"
			disabled={filterHasBeenAdded('fileChangeType') || true}
			onclick={() => {
				handleAddFilter('fileChangeType');
			}}
		/>
		<ContextMenuItem
			icon="tag"
			label="Work catergory (coming soon)"
			disabled={filterHasBeenAdded('semanticType') || true}
			onclick={() => {
				handleAddFilter('semanticType');
			}}
		/>
	</ContextMenuSection>
	{#if addEmpty}
		<ContextMenuSection>
			<ContextMenuItem icon="arrow-right" label="Assign all to branch" onclick={handleAddEmpty} />
		</ContextMenuSection>
	{/if}
</ContextMenu>
