<script lang="ts">
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { MergeMethod } from '$lib/github/types';
	import { persisted, type Persisted } from '$lib/persisted/persisted';
	import DropDownButton from '$lib/shared/DropDownButton.svelte';
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

	let dropDown: DropDownButton;

	const labels = {
		[MergeMethod.Merge]: 'Merge pull request',
		[MergeMethod.Rebase]: 'Rebase and merge',
		[MergeMethod.Squash]: 'Squash and merge'
	};
</script>

<DropDownButton
	style="ghost"
	outline
	{loading}
	bind:this={dropDown}
	{wide}
	{help}
	{disabled}
	on:click={() => {
		dispatch('click', { method: $action });
	}}
>
	{labels[$action]}
	<ContextMenu slot="context-menu">
		<ContextMenuSection>
			{#each Object.values(MergeMethod) as method}
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
