<script lang="ts">
	import { DesktopRedux } from '$lib/redux/store.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { stacksToTabs } from '$lib/tabs/mapping';
	import { getContext } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		selectedId: string | undefined;
	};
	const { projectId, selectedId }: Props = $props();

	const stackService = getContext(StackService);
	const redux = getContext(DesktopRedux);

	const { data: stacks } = $derived(stackService.select(projectId)(redux.rootState$));
	const tabs = $derived(stacksToTabs(stacks));

	stackService.poll(projectId);
</script>

<div class="tabs">
	{#each tabs as tab, i (tab.name)}
		{@const first = i === 0}
		{@const last = i === tabs.length - 1}
		<div class="tab" class:first class:last class:selected={tab.id === selectedId}>
			{tab.name}
		</div>
	{/each}
	<button type="button" class="new-stack" onclick={() => stackService.new(projectId)}>+</button>
</div>

<style lang="postcss">
	.tabs {
		display: flex;
	}

	.tab {
		padding: 12px 14px;
		background: var(--clr-stack-tab-inactive);
		border: 1px solid var(--clr-border-2);
		border-right: none;
		border-bottom: none;
		&.first {
			border-radius: 10px 0 0 0;
		}
	}

	.new-stack {
		border: 1px solid var(--clr-border-2);
		border-bottom: none;
		border-radius: 0 10px 0 0;
		padding: 14px 20px;
	}

	.selected {
		background-color: cadetblue;
		background: var(--clr-stack-tab-active);
	}
</style>
