<script lang="ts">
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { persisted, type Persisted } from '$lib/persisted/persisted';
	import DropDownButton from '$lib/shared/DropDownButton.svelte';
	import { createEventDispatcher } from 'svelte';

	const Action = {
		Create: 'create',
		Draft: 'draft'
	} as const;

	type Action = (typeof Action)[keyof typeof Action];

	const dispatch = createEventDispatcher<{ exec: { action: Action } }>();
	const action = defaultAction();

	export let loading = false;
	let dropDown: DropDownButton;

	const labels = {
		[Action.Create]: 'Create PR',
		[Action.Draft]: 'Create Draft PR'
	};

	function defaultAction(): Persisted<Action> {
		const key = 'projectDefaultPrAction';
		return persisted<Action>(Action.Create, key);
	}
</script>

<DropDownButton
	style="ghost"
	outline
	{loading}
	bind:this={dropDown}
	on:click={() => {
		dispatch('exec', { action: $action });
	}}
>
	{labels[$action]}
	<ContextMenu slot="context-menu">
		<ContextMenuSection>
			{#each Object.values(Action) as method}
				<ContextMenuItem
					label={labels[method]}
					on:click={() => {
						$action = method;
						dropDown.close();
					}}
				/>
			{/each}
		</ContextMenuSection>
	</ContextMenu>
</DropDownButton>
