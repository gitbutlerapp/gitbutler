<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import { focusable } from '$lib/focus/focusable';
	import type { Snippet } from 'svelte';

	type Props = {
		actions: Snippet;
		content: Snippet;
	};
	const { actions, content }: Props = $props();
</script>

<div class="sidebar" use:focusable={{ list: true }}>
	<div class="sidebar-header" use:focusable>
		<p class="text-14 text-semibold">Current sessions</p>
		<div class="sidebar-header-actions">
			{@render actions()}
		</div>
	</div>

	<ConfigurableScrollableContainer>
		<div class="sidebar-content" use:focusable>
			{@render content()}
		</div>
	</ConfigurableScrollableContainer>
</div>

<style lang="postcss">
	.sidebar {
		display: flex;
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

	.sidebar-content {
		display: flex;
		position: relative;
		flex-direction: column;
		padding: 12px;
		gap: 8px;
	}
</style>
