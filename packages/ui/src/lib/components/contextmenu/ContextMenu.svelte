<script lang="ts">
	import { setContext } from 'svelte';
	import type { ContextMenuContext, ContextMenuType } from './contextMenu';

	export let type: ContextMenuType = 'normal';

	const context: ContextMenuContext = { type };
	setContext<ContextMenuContext>('ContextMenu', context);
</script>

<div class="context-menu">
	<slot />
</div>

<style lang="postcss">
	.context-menu {
		display: flex;
		flex-direction: column;
		background: var(--clr-theme-container-pale);
		padding: var(--space-8);
		gap: var(--space-2);
		border: 1px solid var(--clr-theme-container-outline-light);
		border-radius: var(--radius-m);
	}
</style>
