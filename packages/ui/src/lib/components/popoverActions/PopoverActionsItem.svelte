<script lang="ts">
	import Icon from '$components/Icon.svelte';
	import Tooltip from '$components/Tooltip.svelte';
	import type iconsJson from '$lib/data/icons.json';
	import type { Snippet } from 'svelte';

	interface Props {
		el?: HTMLButtonElement;
		icon?: keyof typeof iconsJson;
		tooltip: string;
		thin?: boolean;
		activated?: boolean;
		disabled?: boolean;
		overrideYScroll?: number;
		onclick: (e: MouseEvent) => void;
		children?: Snippet;
	}

	let {
		el = $bindable(),
		icon,
		tooltip,
		thin,
		activated,
		onclick,
		disabled,
		overrideYScroll,
		children
	}: Props = $props();
</script>

<Tooltip
	disabled={activated || disabled}
	text={tooltip}
	position="top"
	delay={200}
	{overrideYScroll}
>
	<button
		type="button"
		bind:this={el}
		data-clickable="true"
		class="overflow-actions-btn"
		{disabled}
		class:thin
		class:activated
		onclick={(e) => {
			e.preventDefault();
			e.stopPropagation();
			onclick(e);
		}}
		oncontextmenu={(e) => e.preventDefault()}
	>
		{#if icon}
			<div class="overflow-actions-btn__icon">
				<Icon name={icon} />
			</div>
		{/if}
		{#if children}
			{@render children()}
		{/if}
	</button>
</Tooltip>

<style lang="postcss">
	.overflow-actions-btn {
		--label-clr: var(--clr-btn-gray-outline-text);
		--icon-opacity: var(--opacity-btn-icon-outline);
		--btn-bg: var(--clr-bg-1);
		--opacity-btn-bg: 0;
		display: flex;
		align-items: center;
		justify-content: center;

		padding: 3px 5px;
		border: 1px solid var(--clr-border-2);
		border-right: none;
		background: color-mix(
			in srgb,
			var(--btn-bg, transparent),
			var(--clr-btn-gray-outline-bg) calc(var(--opacity-btn-bg, 0) * 100%)
		);

		color: var(--label-clr);
		transition:
			background-color var(--transition-fast),
			opacity var(--transition-fast);

		&:hover:not(:disabled),
		&.activated:not(:disabled) {
			--opacity-btn-bg: var(--opacity-btn-outline-bg-hover);

			.overflow-actions-btn__icon {
				--icon-opacity: var(--opacity-btn-icon-outline-hover);
			}
		}

		&:disabled {
			--icon-opacity: 0.5;
		}
	}

	.overflow-actions-btn.thin {
		padding: 1px 4px;
	}

	.overflow-actions-btn__icon {
		display: flex;
		opacity: var(--icon-opacity);
		pointer-events: none;
	}
</style>
