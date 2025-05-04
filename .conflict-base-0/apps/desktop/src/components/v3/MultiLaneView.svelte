<script lang="ts">
	import BranchLayoutMode, { type Layout } from '$components/v3/BranchLayoutMode.svelte';
	import BranchList from '$components/v3/BranchList.svelte';
	import StackCard from '$components/v3/StackCard.svelte';
	import StackTabMenu from '$components/v3/stackTabs/StackTabMenu.svelte';
	import { persisted } from '@gitbutler/shared/persisted';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Scrollbar from '@gitbutler/ui/scroll/Scrollbar.svelte';
	import type { Stack } from '$lib/stacks/stack';

	type Props = {
		projectId: string;
		stacks: Stack[];
	};

	const { projectId, stacks }: Props = $props();

	let scrollableEl = $state<HTMLElement>();
	let mode = $derived(persisted<Layout>('multi', 'branch-layout'));

	let scrollbar = $state<Scrollbar>();

	$effect(() => {
		// Explicit scrollbar track size update since changing scroll width
		// does not trigger the resize observer, and changing css does not
		// trigger the mutation observer
		if ($mode) scrollbar?.updateTrack();
	});
</script>

<div class="lanes-header">
	<div class="left">
		<h3 class="text-14 text-semibold truncate">Branches in workspace</h3>
		{#if stacks.length > 0}
			<Badge>{stacks.length}</Badge>
		{/if}
	</div>
	<div class="right">
		<BranchLayoutMode bind:mode={$mode} />
	</div>
</div>
<div
	class="lanes hide-native-scrollbar"
	bind:this={scrollableEl}
	class:multi={$mode === 'multi'}
	class:single={$mode === 'single'}
	class:vertical={$mode === 'vertical'}
>
	{#each stacks as stack, i}
		{@const stackName = `Stack ${i + 1}`}
		<div
			class="lane"
			class:multi={$mode === 'multi'}
			class:single={$mode === 'single'}
			class:vertical={$mode === 'vertical'}
		>
			<StackCard {projectId} {stackName}>
				<BranchList {projectId} stackId={stack.id} />
				{#snippet contextMenu()}
					<StackTabMenu {projectId} stackId={stack.id} />
				{/snippet}
			</StackCard>
		</div>
	{/each}
	{#if $mode !== 'vertical'}
		<Scrollbar
			bind:this={scrollbar}
			whenToShow="hover"
			viewport={scrollableEl}
			initiallyVisible
			horz
		/>
	{/if}
</div>

<style lang="postcss">
	.lanes {
		display: flex;
		overflow: hidden;
		height: 100%;
		&.single,
		&.multi {
			gap: 12px;
			overflow-x: auto;
			scroll-snap-type: x mandatory;
			padding-left: 12px;
			padding-right: 12px;
		}
		&.multi {
		}
		&.vertical {
			flex-direction: column;
			overflow-y: auto;
			gap: 24px;
		}
	}

	.lane {
		display: flex;
		&.multi {
			width: 280px;
		}
		flex-direction: column;
		flex-shrink: 0;
		scroll-snap-align: start;
		&.single {
			flex-basis: 100%;
		}
	}

	.lanes-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		border-bottom: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
		padding: 6px 12px;
		margin-bottom: 12px;
		& .left {
			display: flex;
			align-items: center;
			gap: 6px;
		}
	}
</style>
