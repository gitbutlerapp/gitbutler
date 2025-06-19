<script lang="ts">
	import ContextMenu from '$components/v3/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';

	type Props = {
		items: {
			label: string;
			onclick: () => void;
		}[][];
		target: HTMLElement | undefined;
		open: boolean;
	};

	let { items, target, open = $bindable<boolean>() }: Props = $props();
</script>

{#if open && target}
	<ContextMenu position={{ element: target }} onclose={() => (open = false)}>
		{#each items as section}
			<ContextMenuSection>
				{#each section as item}
					<ContextMenuItem
						label={item.label}
						onclick={() => {
							item.onclick();
							open = false;
						}}
					/>
				{/each}
			</ContextMenuSection>
		{/each}
	</ContextMenu>
{/if}
