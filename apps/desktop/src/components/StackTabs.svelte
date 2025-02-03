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
	$inspect('getStacks', result);

	const idSelection = getContext(IdSelection);
	const selectedFiles = $derived(idSelection.values());

	// TODO: Get reactive max allowed space for tabs in order to
	// pin "+" button on the right side of page and add shadow on left
	// side to indicate more content, scrolling of tabs, etc. etc.
	const tabOverflow = $derived((result.current.data?.length ?? 0) >= 9);
</script>

<div class="tabs">
	<div class="tabs__available" class:overflow={tabOverflow}>
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
					{@const selected = tab.id === selectedId}
					<StackTab {projectId} {tab} {first} {last} {selected} {tabOverflow} />
				{/each}
			{/snippet}
			{#snippet empty()}
				no stacks
			{/snippet}
		</ReduxResult>
	</div>
	<StackTabNew {projectId} {selectedId} {tabOverflow} />
</div>

<style lang="postcss">
	.tabs {
		display: flex;
	}

	.tabs__available {
		display: flex;
		width: calc(100% - 62px);

		&.overflow {
			width: 100%;
			overflow-x: scroll;
		}
	}

	/* TODO: Replace with <ScrollContainer /> */
	.tabs__available::-webkit-scrollbar {
		display: none;
	}
</style>
