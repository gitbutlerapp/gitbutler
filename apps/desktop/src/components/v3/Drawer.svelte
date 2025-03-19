<script lang="ts">
	import Resizer from '$components/Resizer.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import type { Snippet } from 'svelte';

	type Props = {
		header?: Snippet;
		children: Snippet;
	};

	const { header, children }: Props = $props();

	const [uiState] = inject(UiState);

	let height = $derived(uiState.global.drawerHeight.get());
	let drawerDiv = $state<HTMLDivElement>();
</script>

<div class="drawer" bind:this={drawerDiv} style:height={height.current + 'rem'}>
	<div class="drawer-header">
		<div class="drawer-header__main">
			{#if header}
				{@render header()}
			{/if}
		</div>

		<div class="drawer-header__actions">bla</div>
	</div>

	{#if children}
		{@render children()}
	{/if}

	<Resizer
		direction="up"
		viewport={drawerDiv}
		minHeight={11}
		onHeight={(value) => uiState.global.drawerHeight.set(value)}
	/>
</div>

<style>
	.drawer {
		overflow: hidden;
		position: relative;
		display: flex;
		flex-direction: column;
		flex-shrink: 0;
		flex-grow: 1;
		width: 100%;
		padding: 14px;
		box-sizing: border-box;

		border-radius: var(--radius-ml);
		border: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
	}

	.drawer-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 12px;
	}

	.drawer-header__main {
		flex-grow: 1;
		display: flex;
		gap: 8px;
	}

	.drawer-header__actions {
		flex-shrink: 0;
		display: flex;
		gap: 8px;
	}
</style>
