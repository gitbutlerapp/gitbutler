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
	const { data, status, error } = $derived(stackService.getAll(projectId));
	const tabs = $derived(stacksToTabs(data));
</script>

<div class="tabs">
	<ReduxResult data={tabs} {status} {error}>
		{#snippet children(tabs)}
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
	<StackTabNew {projectId} />
</div>

<style lang="postcss">
	.tabs {
		display: flex;
	}
</style>
