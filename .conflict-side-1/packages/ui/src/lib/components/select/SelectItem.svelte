<script lang="ts">
	import Icon from '$components/Icon.svelte';
	import { focusable } from '$lib/focus/focusable';
	import type iconsJson from '$lib/data/icons.json';
	import type { Snippet } from 'svelte';

	interface Props {
		icon?: keyof typeof iconsJson | undefined;
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
		iconSnippet,
		selected = false,
		disabled = false,
		loading = false,
		highlighted = false,
		value = undefined,
		testId,
		onClick,
		children
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
	{#if icon || selected}
		<div class="icon">
			{#if icon}
				<Icon name={loading ? 'spinner' : icon} />
			{:else}
				<Icon name="tick" />
			{/if}
		</div>
	{/if}
</button>

<style lang="postcss">
	.select-button {
		display: flex;
		align-items: center;
		width: 100%;
		padding: 8px;
		gap: 10px;
		border-radius: var(--radius-m);
		color: var(--clr-scale-ntrl-10);
		white-space: nowrap;
		user-select: none;
		&:not(.selected):hover:enabled,
		&:not(.selected):focus:enabled {
			background-color: var(--clr-bg-1-muted);
			& .icon {
				color: var(--clr-scale-ntrl-40);
			}
		}
		&:disabled {
			opacity: 0.4;
		}
		& .icon,
		.custom-icon {
			display: flex;
			flex-shrink: 0;
			color: var(--clr-text-2);
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
		background-color: var(--clr-bg-2);

		& .label {
			opacity: 0.5;
		}
	}

	.highlighted {
		background-color: var(--clr-bg-1-muted);
	}
</style>
