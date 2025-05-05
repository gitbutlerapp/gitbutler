<script lang="ts">
	import BranchLayoutMode, { type Layout } from '$components/v3/BranchLayoutMode.svelte';
	import BranchList from '$components/v3/BranchList.svelte';
	import StackCard from '$components/v3/StackCard.svelte';
	import StackDraft from '$components/v3/StackDraft.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
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

	const [uiState] = inject(UiState);
	const drawer = $derived(uiState.project(projectId).drawerPage);
	const isCommitting = $derived(drawer.current === 'new-commit');
</script>

<div class="lanes">
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
		class="lanes-content hide-native-scrollbar dotted"
		bind:this={scrollableEl}
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
					<StackCard {projectId}>
						<BranchList {projectId} stackId={stack.id} />
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
		{:else if isCommitting}
			<StackDraft {projectId} />
		{/if}
	</div>
</div>

<style lang="postcss">
	.lanes {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		overflow: hidden;
		height: 100%;
	}
	.lanes-content {
		display: flex;
		overflow: hidden;
		height: 100%;
		&.single,
		&.multi {
			overflow-x: auto;
			scroll-snap-type: x mandatory;
		}
		&.multi {
		}
		&.vertical {
			flex-direction: column;
			overflow-y: auto;
			gap: 6;
		}
	}

	.lane {
		display: flex;
		padding: 12px;
		&.multi {
			width: 280px;
		}
		flex-direction: column;
		flex-shrink: 0;
		scroll-snap-align: start;
		&.single {
			flex-basis: 100%;
		}
		--menu-btn-size: 20px;
	}

	.lanes-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		border-bottom: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
		padding: 6px 12px;
		& .left {
			display: flex;
			align-items: center;
			gap: 6px;
		}
	}

	.dotted {
		background-image: radial-gradient(
			oklch(from var(--clr-scale-ntrl-50) l c h / 0.5) 0.6px,
			#ffffff00 0.6px
		);
		background-size: 6px 6px;
	}
</style>
