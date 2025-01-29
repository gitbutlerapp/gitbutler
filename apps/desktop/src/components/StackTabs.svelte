<script lang="ts">
	import ReduxResult from './ReduxResult.svelte';
	import StackTab from './StackTab.svelte';
	import StackTabNew from './StackTabNew.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { stacksToTabs } from '$lib/tabs/mapping';
	import { getContext } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		selectedId: string | undefined;
	};
	const { projectId, selectedId }: Props = $props();

	const stackService = getContext(StackService);
	const result = $derived(stackService.getStacks(projectId));
</script>

<div class="tabs">
	<ReduxResult result={result.current}>
		{#snippet children(result)}
			{@const tabs = stacksToTabs(result)}
			{#each tabs as tab, i (tab.name)}
				{@const first = i === 0}
				{@const last = i === tabs.length - 1}
				{@const selected = tab.id === selectedId}
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
