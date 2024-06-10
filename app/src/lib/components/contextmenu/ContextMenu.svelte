<script lang="ts">
	import { createEventDispatcher, onMount, setContext } from 'svelte';
	import { writable } from 'svelte/store';
	import type { ContextMenuContext, ContextMenuItem, ContextMenuType } from './contextMenu';

	export let type: ContextMenuType = 'normal';
	export const selection = writable<ContextMenuItem | undefined>(undefined);

	const context: ContextMenuContext = { type, selection };
	setContext<ContextMenuContext>('ContextMenu', context);

	const dispatch = createEventDispatcher<{ select: ContextMenuItem | undefined }>();

	onMount(() => {
		const unsubscribe = selection.subscribe((value) => dispatch('select', value));
		return () => {
			unsubscribe();
		};
	});
</script>

<div class="context-menu">
	<slot />
</div>

<style lang="postcss">
	.context-menu {
		display: flex;
		flex-direction: column;
		background: var(--clr-bg-2);
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		box-shadow: var(--fx-shadow-s);
	}
</style>
