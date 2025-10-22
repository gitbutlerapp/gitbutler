<script lang="ts">
	import { Button } from '@gitbutler/ui';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import type { Snippet } from 'svelte';

	interface Props {
		content: Snippet;
		actions?: Snippet<[element: HTMLElement]>;
		headerHeight?: number;
		onclose?: () => void;
	}

	let { content, actions, headerHeight = $bindable(), onclose }: Props = $props();

	let headerDiv = $state<HTMLDivElement>();
</script>

<div bind:this={headerDiv} class="drawer-header" bind:clientHeight={headerHeight} use:focusable>
	<div class="drawer-header__title">
		{@render content()}
	</div>

	{#if actions || onclose}
		<div class="drawer-header__actions actions-with-separators">
			{#if actions}
				{@render actions(headerDiv)}
			{/if}

			{#if onclose && actions}
				<div class="divider"></div>
			{/if}

			{#if onclose}
				<Button kind="ghost" icon="cross" size="tag" onclick={() => onclose()} />
			{/if}
		</div>
	{/if}
</div>

<style lang="postcss">
	.drawer-header {
		display: flex;
		position: relative;
		flex-shrink: 0;
		align-items: center;
		justify-content: space-between;
		height: 42px;
		padding: 0 12px 0 14px;
		gap: 8px;
		border-bottom: 1px solid transparent;
		border-bottom-color: var(--clr-border-2);
		background-color: var(--clr-bg-2);
	}

	.drawer-header__title {
		display: flex;
		flex-grow: 1;
		align-items: center;
		height: 100%;
		overflow: hidden;
		gap: 6px;
	}

	.drawer-header__actions {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		margin-right: -2px; /* buttons have some paddings that look not aligned. With this we "remove" them */
		gap: 10px;
	}

	.drawer-header__actions :global(.divider) {
		width: 1px;
		height: 18px;
		background-color: var(--clr-border-2);
	}
</style>
