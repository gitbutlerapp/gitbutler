<script lang="ts">
	// import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import Resizer from '$components/Resizer.svelte';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import type { Snippet } from 'svelte';

	type Props = {
		actions: Snippet;
		content: Snippet;
	};
	const { actions, content }: Props = $props();

	let sidebarViewportRef = $state<HTMLDivElement>();
</script>

<div class="sidebar" bind:this={sidebarViewportRef} use:focusable={{ list: true }}>
	<div class="sidebar-header" use:focusable>
		<p class="text-14 text-semibold">Current sessions</p>
		<div class="sidebar-header-actions">
			{@render actions()}
		</div>
	</div>

	{@render content()}

	<Resizer
		direction="right"
		viewport={sidebarViewportRef}
		defaultValue={20}
		minWidth={20}
		maxWidth={35}
		persistId="resizer-codegenLeft"
	/>
</div>

<style lang="postcss">
	.sidebar {
		display: flex;
		position: relative;
		flex-shrink: 0;
		flex-direction: column;

		/* TODO: This should be resizable */
		width: 320px;
		height: 100%;

		overflow: hidden;

		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-l);
		background-color: var(--clr-bg-1);
	}

	.sidebar-header {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		align-self: stretch;
		justify-content: space-between;
		height: 40px;
		padding: 0 10px 0 14px;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);
	}

	.sidebar-header-actions {
		display: flex;
		align-items: center;
		gap: 8px;
	}
</style>
