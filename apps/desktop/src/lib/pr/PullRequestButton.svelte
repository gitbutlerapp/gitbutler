<script lang="ts">
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { persisted } from '$lib/persisted/persisted';
	import DropDownButton from '$lib/shared/DropDownButton.svelte';

	enum Action {
		Create = 'createPr',
		CreateDraft = 'createDraftPr'
	}
	const actions = Object.values(Action);
	const labels = {
		[Action.Create]: 'Create PR',
		[Action.CreateDraft]: 'Create Draft PR'
	};

	type Props = {
		loading: boolean;
		disabled?: boolean;
		tooltip?: string;
		click: (opts: { draft: boolean }) => void;
	};
	const { loading, disabled, tooltip, click }: Props = $props();

	const preferredAction = persisted<Action>(Action.Create, 'projectDefaultPrAction');
	let dropDown = $state<ReturnType<typeof DropDownButton>>();

	$effect(() => {
		if (!Object.values(Action).includes($preferredAction)) {
			$preferredAction = Action.Create;
		}
	});
</script>

<DropDownButton
	style="ghost"
	outline
	{tooltip}
	{disabled}
	{loading}
	bind:this={dropDown}
	onclick={() => click({ draft: $preferredAction === Action.CreateDraft })}
>
	{labels[$preferredAction]}
	{#snippet contextMenuSlot()}
		<ContextMenuSection>
			{#each actions as method}
				<ContextMenuItem
					label={labels[method]}
					on:click={() => {
						preferredAction.set(method);
						dropDown?.close();
					}}
				/>
			{/each}
		</ContextMenuSection>
	{/snippet}
</DropDownButton>
