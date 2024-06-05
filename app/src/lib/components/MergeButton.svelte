<script lang="ts">
	import DropDownButton from '$lib/components/DropDownButton.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { MergeMethod } from '$lib/github/types';
	import { persisted, type Persisted } from '$lib/persisted/persisted';
	import * as toasts from '$lib/utils/toasts';
	import { createEventDispatcher } from 'svelte';

	export let projectId: string;
	export let loading = false;
	export let disabled = false;
	export let wide = false;
	export let help = '';

	function persistedAction(projectId: string): Persisted<MergeMethod> {
		const key = 'projectMergeMethod';
		return persisted<MergeMethod>(MergeMethod.Merge, key + projectId);
	}

	const dispatch = createEventDispatcher<{ click: { method: MergeMethod } }>();
	const action = persistedAction(projectId);

	let contextMenu: ContextMenu;
	let dropDown: DropDownButton;

	$: selection$ = contextMenu?.selection$;
</script>

<DropDownButton
	style="ghost"
	kind="solid"
	{loading}
	bind:this={dropDown}
	{wide}
	{help}
	{disabled}
	on:click={() => {
		dispatch('click', { method: $action });
	}}
>
	{$selection$?.label}
	<ContextMenu
		type="select"
		slot="context-menu"
		bind:this={contextMenu}
		on:select={(e) => {
			// TODO: Refactor to use generics if/when that works with Svelte
			switch (e.detail?.id) {
				case MergeMethod.Merge:
					$action = MergeMethod.Merge;
					break;
				case MergeMethod.Rebase:
					$action = MergeMethod.Rebase;
					break;
				case MergeMethod.Squash:
					$action = MergeMethod.Squash;
					break;
				default:
					toasts.error('Unknown merge method');
			}
			dropDown.close();
		}}
	>
		<ContextMenuSection>
			<ContextMenuItem
				id={MergeMethod.Merge}
				label="Merge pull request"
				selected={$action === MergeMethod.Merge}
			/>
			<ContextMenuItem
				id={MergeMethod.Rebase}
				label="Rebase and merge"
				selected={$action === MergeMethod.Rebase}
			/>
			<ContextMenuItem
				id={MergeMethod.Squash}
				label="Squash and merge"
				selected={$action === MergeMethod.Squash}
			/>
		</ContextMenuSection>
	</ContextMenu>
</DropDownButton>
