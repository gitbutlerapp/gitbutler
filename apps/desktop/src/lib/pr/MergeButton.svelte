<script lang="ts">
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { MergeMethod } from '$lib/forge/interface/types';
	import DropDownButton from '$lib/shared/DropDownButton.svelte';
	import { persisted, type Persisted } from '@gitbutler/shared/persisted';
	import { createEventDispatcher } from 'svelte';

	interface Props {
		projectId: string;
		loading?: boolean;
		disabled?: boolean;
		wide?: boolean;
		tooltip?: string;
	}

	let {
		projectId,
		loading = false,
		disabled = false,
		wide = false,
		tooltip = ''
	}: Props = $props();

	function persistedAction(projectId: string): Persisted<MergeMethod> {
		const key = 'projectMergeMethod';
		return persisted<MergeMethod>(MergeMethod.Merge, key + projectId);
	}

	const dispatch = createEventDispatcher<{ click: { method: MergeMethod } }>();
	const action = persistedAction(projectId);

	let dropDown: ReturnType<typeof DropDownButton> | undefined = $state();

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
	{tooltip}
	{disabled}
	onclick={() => {
		dispatch('click', { method: $action });
	}}
>
	{labels[$action]}
	{#snippet contextMenuSlot()}
		<ContextMenuSection>
			{#each Object.values(MergeMethod) as method}
				<ContextMenuItem
					label={labels[method]}
					onclick={() => {
						$action = method;
						dropDown?.close();
					}}
				/>
			{/each}
		</ContextMenuSection>
	{/snippet}
</DropDownButton>
