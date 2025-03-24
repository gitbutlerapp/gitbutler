<script lang="ts">
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';

	type Props = {
		onTypeSuggestions: boolean;
		toggleOnTypeSuggestions: () => void;
		menu: ReturnType<typeof ContextMenu> | undefined;
		leftClickTrigger: HTMLElement | undefined;
	};

	let {
		onTypeSuggestions,
		toggleOnTypeSuggestions,
		menu = $bindable(),
		leftClickTrigger
	}: Props = $props();
</script>

<ContextMenu bind:this={menu} {leftClickTrigger}>
	<ContextMenuSection>
		<ContextMenuItem label="On-type suggestions" onclick={toggleOnTypeSuggestions}>
			{#snippet control()}
				<Tooltip
					text={onTypeSuggestions
						? 'Suggestions will be generated as you type'
						: 'Suggestions will only be generated when you press ⌘ G'}
				>
					<Toggle small checked={onTypeSuggestions} onclick={toggleOnTypeSuggestions} />
				</Tooltip>
			{/snippet}
		</ContextMenuItem>
	</ContextMenuSection>
</ContextMenu>
