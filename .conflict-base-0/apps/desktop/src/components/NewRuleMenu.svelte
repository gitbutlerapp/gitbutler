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

	export function open(e: MouseEvent) {
		contextMenu?.open(e);
	}
</script>

<ContextMenu bind:this={contextMenu} rightClickTrigger={trigger}>
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
			label="Contains text"
			disabled={filterHasBeenAdded('contentMatchesRegex')}
			onclick={() => {
				handleAddFilter('contentMatchesRegex');
			}}
		/>
		<ContextMenuItem
			icon="file-changes"
			label="Change type"
			disabled={filterHasBeenAdded('fileChangeType')}
			onclick={() => {
				handleAddFilter('fileChangeType');
			}}
		/>
		<ContextMenuItem
			icon="tag"
			label="Work catergory"
			disabled={filterHasBeenAdded('semanticType')}
			onclick={() => {
				handleAddFilter('semanticType');
			}}
		/>
	</ContextMenuSection>
	{#if addEmpty}
		<ContextMenuSection>
			<ContextMenuItem label="Assign all to branch" onclick={handleAddEmpty} />
		</ContextMenuSection>
	{/if}
</ContextMenu>
