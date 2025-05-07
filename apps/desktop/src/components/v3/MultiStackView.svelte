<script lang="ts">
	import BranchLayoutMode, { type Layout } from '$components/v3/BranchLayoutMode.svelte';
	import BranchList from '$components/v3/BranchList.svelte';
	import MultiStackCreateNew from '$components/v3/MultiStackCreateNew.svelte';
	import StackDraft from '$components/v3/StackDraft.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Scrollbar from '@gitbutler/ui/scroll/Scrollbar.svelte';
	import type { Stack } from '$lib/stacks/stack';

	type Props = {
		projectId: string;
		selectedId?: string;
		stacks: Stack[];
	};

	const { projectId, selectedId, stacks }: Props = $props();

	let lanesEl = $state<HTMLElement>();
	let mode = $derived(persisted<Layout>('multi', 'branch-layout'));

	let scrollbar = $state<Scrollbar>();

	$effect(() => {
		// Explicit scrollbar track size update since changing scroll width
		// does not trigger the resize observer, and changing css does not
		// trigger the mutation observer
		if ($mode) scrollbar?.updateTrack();
	});

	const [uiState] = inject(UiState);
	const drawer = $derived(uiState.project(projectId).drawerPage);
	const isCommitting = $derived(drawer.current === 'new-commit');
</script>

<div class="lanes">
	<div class="lanes-header">
		<div class="title">
			<h3 class="text-14 text-semibold truncate">Applied branches</h3>
			{#if stacks.length > 0}
				<Badge>{stacks.length}</Badge>
			{/if}
		</div>
		<div class="actions">
			<BranchLayoutMode bind:mode={$mode} />
		</div>

		<MultiStackCreateNew {projectId} stackId={selectedId} noStacks={stacks.length === 0} />
	</div>

	<div
		class="lanes-content hide-native-scrollbar dotted-pattern"
		bind:this={lanesEl}
		class:multi={$mode === 'multi'}
		class:single={$mode === 'single'}
		class:vertical={$mode === 'vertical'}
	>
		{#if stacks.length > 0}
			{#each stacks as stack}
				<div
					class="lane"
					class:multi={$mode === 'multi'}
					class:single={$mode === 'single'}
					class:vertical={$mode === 'vertical'}
				>
					<BranchList {projectId} stackId={stack.id} />
				</div>
			{/each}
		{:else if isCommitting}
			<StackDraft {projectId} />
		{/if}

		{#if $mode !== 'vertical'}
			<Scrollbar whenToShow="hover" viewport={lanesEl} horz />
		{/if}
	</div>
</div>

<style lang="postcss">
	.lanes {
		position: relative;
		display: flex;
		flex-direction: column;
		flex: 1;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		overflow: hidden;
	}

	.lanes-header {
		display: flex;
		justify-content: space-between;
		gap: 10px;
		align-items: center;
		border-bottom: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
		height: 44px;
		padding-left: 12px;

		& .title {
			flex: 1;
			display: flex;
			align-items: center;
			gap: 6px;
		}

		& .actions {
			display: flex;
		}
	}

	.lanes-content {
		display: flex;
		height: 100%;
		margin: 0 -1px;

		&.single {
			scroll-snap-type: x mandatory;
		}
		&.single,
		&.multi {
			overflow-x: auto;
		}
		&.vertical {
			flex-direction: column;
			overflow-y: auto;
		}
	}

	.lane {
		position: relative;
		display: flex;
		flex-direction: column;
		flex-shrink: 0;
		scroll-snap-align: start;
		border-right: 1px solid var(--clr-border-2);
		overflow-x: hidden;
		overflow-y: auto;

		&:first-child {
			border-left: 1px solid var(--clr-border-2);
		}

		&.single {
			flex-basis: calc(100% - 30px);
		}
		&.multi {
			width: 340px;
		}
	}
</style>
