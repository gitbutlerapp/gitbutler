<script lang="ts" context="module">
	export enum Action {
		Create = 'createPr',
		CreateDraft = 'createDraftPr'
	}

	const actions = Object.values(Action);
	const labels = {
		[Action.Create]: 'Create PR',
		[Action.CreateDraft]: 'Create Draft PR'
	};
</script>

<script lang="ts">
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { persisted } from '$lib/persisted/persisted';
	import DropDownButton from '$lib/shared/DropDownButton.svelte';

	type Props = { loading: boolean; disabled: boolean; click: (opts: { draft: boolean }) => void };
	const { loading, disabled, click }: Props = $props();

	const preferredAction = persisted<Action>(Action.Create, 'projectDefaultPrAction');
	let dropDown: DropDownButton;
</script>

<DropDownButton
	style="ghost"
	outline
	{disabled}
	{loading}
	bind:this={dropDown}
	on:click={() => click({ draft: $preferredAction === Action.CreateDraft })}
>
	{labels[$preferredAction]}
	<ContextMenuSection slot="context-menu">
		{#each actions as method}
			<ContextMenuItem
				label={labels[method]}
				on:click={() => {
					preferredAction.set(method);
					dropDown.close();
				}}
			/>
		{/each}
	</ContextMenuSection>
</DropDownButton>
