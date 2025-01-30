<script lang="ts">
	import ReduxResult from './ReduxResult.svelte';
	import StackTab from './StackTab.svelte';
	import StackTabNew from './StackTabNew.svelte';
	import StackTabPreview from './StackTabPreview.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { stacksToTabs } from '$lib/tabs/mapping';
	import { getContext } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		selectedId: string | undefined;
		previewing: boolean;
	};
	const { projectId, selectedId, previewing }: Props = $props();

	const stackService = getContext(StackService);
	const result = $derived(stackService.getStacks(projectId));

	const idSelection = getContext(IdSelection);
	const selectedFiles = $derived(idSelection.values());
</script>

<div class="tabs">
	{#if selectedFiles.length > 0}
		<StackTabPreview
			{projectId}
			count={selectedFiles.length}
			selected={previewing || selectedId === undefined}
		/>
	{/if}

	<ReduxResult result={result.current}>
		{#snippet children(result)}
			{@const tabs = stacksToTabs(result)}
			{#each tabs as tab, i (tab.name)}
				{@const first = i === 0}
				{@const last = i === tabs.length - 1}
				{@const selected = !previewing && tab.id === selectedId}
				<StackTab {projectId} {tab} {first} {last} {selected} />
			{/each}
		{/snippet}
		{#snippet empty()}
			no stacks
		{/snippet}
	</ReduxResult>
	<StackTabNew {projectId} {selectedId} />
</div>

<style lang="postcss">
	.tabs {
		display: flex;
	}
</style>
