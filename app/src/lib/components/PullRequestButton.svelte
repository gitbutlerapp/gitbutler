<script lang="ts">
	import DropDownButton from './DropDownButton.svelte';
	import ContextMenu from './contextmenu/ContextMenu.svelte';
	import ContextMenuItem from './contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from './contextmenu/ContextMenuSection.svelte';
	import { persisted, type Persisted } from '$lib/persisted/persisted';
	import * as toasts from '$lib/utils/toasts';
	import { createEventDispatcher } from 'svelte';

	enum Action {
		Create = 'create',
		Draft = 'draft'
	}

	const dispatch = createEventDispatcher<{ click: { action: Action } }>();
	const action = defaultAction();

	export let loading = false;
	let dropDown: DropDownButton;
	let contextMenu: ContextMenu;

	$: selection = contextMenu?.selection;

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
		dispatch('click', { action: $action });
	}}
>
	{$selection?.label}
	<ContextMenu
		type="select"
		slot="context-menu"
		bind:this={contextMenu}
		on:select={(e) => {
			// TODO: Refactor to use generics if/when that works with Svelte
			switch (e.detail?.id) {
				case Action.Create:
					$action = Action.Create;
					break;
				case Action.Draft:
					$action = Action.Draft;
					break;
				default:
					toasts.error('Unknown merge method');
			}
			dropDown.close();
		}}
	>
		<ContextMenuSection>
			<ContextMenuItem id={Action.Create} label="Create PR" selected={$action === Action.Create} />
			<ContextMenuItem
				id={Action.Draft}
				label="Create Draft PR"
				selected={$action === Action.Draft}
			/>
		</ContextMenuSection>
	</ContextMenu>
</DropDownButton>
