<script lang="ts">
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { MergeMethod } from '$lib/forge/interface/types';
	import DropDownButton from '$lib/shared/DropDownButton.svelte';
	import { persisted, type Persisted } from '@gitbutler/shared/persisted';
	import type { Props as ButtonProps } from '@gitbutler/ui/Button.svelte';

	interface Props {
		projectId: string;
		onclick: (method: MergeMethod) => void;
		loading?: boolean;
		disabled?: boolean;
		wide?: boolean;
		tooltip?: string;
		style?: ButtonProps['style'];
		kind?: ButtonProps['kind'];
	}

	const {
		projectId,
		onclick,
		loading = false,
		disabled = false,
		wide = false,
		tooltip = '',
		style = 'neutral',
		kind = 'outline'
	}: Props = $props();

	function persistedAction(projectId: string): Persisted<MergeMethod> {
		const key = 'projectMergeMethod';
		return persisted<MergeMethod>(MergeMethod.Merge, key + projectId);
	}

	const action = persistedAction(projectId);

	let dropDown: ReturnType<typeof DropDownButton> | undefined;

	const labels = {
		[MergeMethod.Merge]: 'Merge pull request',
		[MergeMethod.Rebase]: 'Rebase and merge',
		[MergeMethod.Squash]: 'Squash and merge'
	};
</script>

<DropDownButton
	bind:this={dropDown}
	onclick={() => onclick?.($action)}
	{style}
	{kind}
	{loading}
	{wide}
	{tooltip}
	{disabled}
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
