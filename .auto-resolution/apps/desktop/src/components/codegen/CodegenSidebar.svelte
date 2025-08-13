<script lang="ts">
	import type { Snippet } from 'svelte';

	type Props = {
		actions: Snippet;
		content: Snippet;
	};
	const { actions, content }: Props = $props();
</script>

<div class="sidebar">
	<div class="sidebar-header">
		<p class="text-14 text-semibold">Current sessions</p>
		<div class="sidebar-header-actions">
			{@render actions()}
		</div>
	</div>
	<div class="sidebar-content">
		{@render content()}
	</div>
</div>

<style lang="postcss">
	.sidebar {
		display: flex;
		flex-shrink: 0;
		flex-direction: column;

		/* TODO: This should be resizable */
		width: 350px;
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
		flex-grow: 1;
		flex-direction: column;
		align-items: flex-start;
		align-self: stretch;
		padding: 12px;

		overflow: auto;
		gap: 8px;
	}
</style>
