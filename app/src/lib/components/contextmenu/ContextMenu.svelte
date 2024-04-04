<script lang="ts">
	import { BehaviorSubject } from 'rxjs';
	import { createEventDispatcher, onDestroy, setContext } from 'svelte';
	import type { ContextMenuContext, ContextMenuItem, ContextMenuType } from './contextMenu';

	export let type: ContextMenuType = 'normal';
	export const selection$ = new BehaviorSubject<ContextMenuItem | undefined>(undefined);

	const context: ContextMenuContext = { type, selection$ };
	setContext<ContextMenuContext>('ContextMenu', context);

	const dispatch = createEventDispatcher<{ select: ContextMenuItem | undefined }>();

	const subscription = selection$.subscribe((item) => dispatch('select', item));
	onDestroy(() => {
		subscription.unsubscribe();
	});
</script>

<div class="context-menu">
	<slot />
</div>

<style lang="postcss">
	.context-menu {
		display: flex;
		flex-direction: column;
		background: var(--clr-container-pale);
		border: 1px solid var(--clr-container-outline-light);
		border-radius: var(--radius-m);
		box-shadow: var(--fx-shadow-s);
	}
</style>
