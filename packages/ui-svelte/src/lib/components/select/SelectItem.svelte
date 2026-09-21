<script lang="ts">
	import Icon from "$components/Icon.svelte";
	import { focusable } from "$lib/focus/focusable";
	import { type IconName } from "$lib/icons/names";
	import type { Snippet } from "svelte";

	interface Props {
		icon?: IconName;
		/** Replaces the trailing icon while the item is hovered or highlighted. */
		hoverIcon?: IconName;
		iconSnippet?: Snippet;
		selected?: boolean;
		disabled?: boolean;
		loading?: boolean;
		highlighted?: boolean;
		value?: string | undefined;
		testId?: string;
		children?: Snippet;
		onClick?: (value: string | undefined) => void;
	}

	const {
		icon = undefined,
		hoverIcon = undefined,
		iconSnippet,
		selected = false,
		disabled = false,
		loading = false,
		highlighted = false,
		value = undefined,
		testId,
		onClick,
		children,
	}: Props = $props();

	let self = $state<HTMLButtonElement>();
</script>

<button
	bind:this={self}
	data-testid={testId}
	type="button"
	{disabled}
	class="select-button"
	class:selected
	class:highlighted
	use:focusable={{ button: true, onAction: () => self?.click() }}
	onclick={() => onClick?.(value)}
>
	{#if iconSnippet}
		<div class="custom-icon">
			{@render iconSnippet()}
		</div>
	{/if}
	<div class="label text-13">
		{@render children?.()}
	</div>
	{#if icon || selected || hoverIcon}
		<div class="icon" class:has-hover-icon={!!hoverIcon}>
			{#if icon || selected}
				<span class="icon__base">
					{#if icon}
						<Icon name={loading ? "spinner" : icon} />
					{:else}
						<Icon name="tick" />
					{/if}
				</span>
			{/if}
			{#if hoverIcon}
				<span class="icon__hover">
					<Icon name={hoverIcon} />
				</span>
			{/if}
		</div>
	{/if}
</button>

<style lang="postcss">
	.select-button {
		display: flex;
		align-items: center;
		width: 100%;
		/* Icons are 16px but a text-13 line box is 15.6px, so rows with and without an
		   icon would otherwise differ in height. Pin it: 16px icon + 8px padding twice. */
		min-height: 32px;
		padding: 8px;
		gap: 10px;
		border-radius: var(--radius-m);
		color: var(--text-1);
		white-space: nowrap;
		user-select: none;
		&:not(.selected):hover:enabled,
		&:not(.selected):focus:enabled {
			background-color: var(--hover-bg-1);
		}
		&:disabled {
			opacity: 0.4;
		}
		& .custom-icon {
			display: flex;
			flex-shrink: 0;
			color: var(--text-2);
		}
		& .label {
			display: block;
			flex: 1;
			overflow: hidden;
			text-align: left;
			text-overflow: ellipsis;
			white-space: nowrap;
		}
	}

	.selected {
		background-color: var(--bg-2);

		& .label {
			opacity: 0.5;
		}
	}

	.highlighted:not(.selected) {
		background-color: var(--hover-bg-1);
	}

	/**
	 * The hover icon takes over the trailing slot while the item is hovered or highlighted.
	 * Both icons are stacked in the same grid cell so swapping them never changes the
	 * slot's width — otherwise the label would shift on hover.
	 */
	.icon {
		display: grid;
		flex-shrink: 0;
		place-items: center;
		color: var(--text-2);

		& .icon__base,
		& .icon__hover {
			display: flex;
			grid-area: 1 / 1;
		}
		& .icon__hover {
			opacity: 0;
		}
	}

	/* Only swap when there is something to swap to, so a plain tick survives hover. */
	.select-button:hover:enabled,
	.select-button.highlighted:enabled {
		& .icon.has-hover-icon .icon__base {
			opacity: 0;
		}
		& .icon.has-hover-icon .icon__hover {
			opacity: 1;
		}
	}
</style>
