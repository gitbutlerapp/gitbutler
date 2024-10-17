<script lang="ts">
	import { getPreferredPRAction, PRAction, PRActionLabels, prActions } from './pr';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import DropDownButton from '$lib/shared/DropDownButton.svelte';

	type Props = {
		loading: boolean;
		disabled?: boolean;
		tooltip?: string;
		click: (opts: { draft: boolean }) => void;
	};
	const { loading, disabled, tooltip, click }: Props = $props();

	const preferredAction = getPreferredPRAction();
	let dropDown = $state<ReturnType<typeof DropDownButton>>();
</script>

<DropDownButton
	style="ghost"
	outline
	{tooltip}
	{disabled}
	{loading}
	bind:this={dropDown}
	onclick={() => click({ draft: $preferredAction === PRAction.CreateDraft })}
>
	{PRActionLabels[$preferredAction]}
	{#snippet contextMenuSlot()}
		<ContextMenuSection>
			{#each prActions as method}
				<ContextMenuItem
					label={PRActionLabels[method]}
					onclick={() => {
						preferredAction.set(method);
						dropDown?.close();
					}}
				/>
			{/each}
		</ContextMenuSection>
	{/snippet}
</DropDownButton>
