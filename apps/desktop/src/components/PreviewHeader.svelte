<script lang="ts">
	import { Button } from "@gitbutler/ui";
	import { focusable } from "@gitbutler/ui/focus/focusable";
	import { onMount } from "svelte";
	import type { Snippet } from "svelte";

	interface Props {
		content: Snippet;
		actions?: Snippet<[element: HTMLElement]>;
		headerHeight?: number;
		transparent?: boolean;
		sticky?: boolean;
		reserveSpaceOnStuck?: boolean;
		closeButtonPlaceholder?: boolean;
		scrollRoot?: HTMLElement | null;
		onclose?: () => void;
		/**
		 * Called when the header is double-clicked.
		 * Typically used to toggle the drawer's collapsed state.
		 */
		ondblclick?: () => void;
	}

	let {
		content,
		actions,
		headerHeight = $bindable(),
		transparent,
		sticky,
		reserveSpaceOnStuck,
		closeButtonPlaceholder,
		scrollRoot,
		onclose,
		ondblclick,
	}: Props = $props();

	let headerDiv = $state<HTMLDivElement>();
	let sentinelDiv = $state<HTMLDivElement>();
	let isStuck = $state(false);

	onMount(() => {
		if (!reserveSpaceOnStuck || !sentinelDiv) return;

		const observer = new IntersectionObserver(
			([entry]) => {
				if (entry) {
					isStuck = !entry.isIntersecting;
				}
			},
			{
				threshold: 1,
				root: scrollRoot || null,
			},
		);

		observer.observe(sentinelDiv);

		return () => {
			observer.disconnect();
		};
	});
</script>

{#if sticky && reserveSpaceOnStuck}
	<div bind:this={sentinelDiv} class="sticky-sentinel"></div>
{/if}

<div
	role="presentation"
	bind:this={headerDiv}
	class="drawer-header"
	class:sticky
	class:stuck={isStuck}
	bind:clientHeight={headerHeight}
	use:focusable
	{ondblclick}
	style:background={transparent ? "transparent" : undefined}
>
	<div class="drawer-header__title">
		{@render content()}
	</div>

	{#if actions || onclose || closeButtonPlaceholder}
		<div class="drawer-header__actions">
			{#if actions}
				<div class="drawer-header__optional-actions">
					{@render actions(headerDiv)}
				</div>
			{/if}

			{#if (onclose && actions) || (closeButtonPlaceholder && isStuck)}
				<div class="divider"></div>
			{/if}

			{#if (closeButtonPlaceholder && !actions) || isStuck}
				<div style="width: 22px; height: var(--size-button);"></div>
			{/if}

			{#if onclose}
				<Button kind="ghost" icon="cross" size="tag" onclick={() => onclose()} />
			{/if}
		</div>
	{/if}
</div>

<style lang="postcss">
	.sticky-sentinel {
		visibility: hidden;
		position: absolute;
		top: -26px;
		width: 1px;
		height: 1px;
		pointer-events: none;
	}

	.drawer-header {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: space-between;
		height: 42px;
		padding: 0 12px 0 14px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);
		transition: box-shadow var(--transition-medium);

		&.sticky {
			z-index: var(--z-ground);
			position: sticky;
			top: 0;
		}

		&.stuck {
			box-shadow: 0 4px 8px rgba(0, 0, 0, 0.06);
		}
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

	.drawer-header__optional-actions {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.drawer-header__actions :global(.divider) {
		width: 1px;
		height: 18px;
		background-color: var(--clr-border-2);
	}
</style>
