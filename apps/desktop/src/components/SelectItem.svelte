<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { Snippet } from 'svelte';

	interface Props {
		icon?: keyof typeof iconsJson | undefined;
		selected?: boolean;
		disabled?: boolean;
		loading?: boolean;
		highlighted?: boolean;
		value?: string | undefined;
		children?: Snippet;
		onClick?: (value: string | undefined) => void;
	}

	const {
		icon = undefined,
		selected = false,
		disabled = false,
		loading = false,
		highlighted = false,
		value = undefined,
		onClick,
		children
	}: Props = $props();
</script>

<button
	type="button"
	{disabled}
	class="button"
	class:selected
	class:highlighted
	onclick={() => onClick?.(value)}
>
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
	.button {
		user-select: none;
		display: flex;
		align-items: center;
		color: var(--clr-scale-ntrl-10);
		font-weight: 700;
		padding: 8px 8px;
		justify-content: space-between;
		border-radius: var(--radius-m);
		width: 100%;
		gap: 16px;
		white-space: nowrap;
		&:not(.selected):hover:enabled,
		&:not(.selected):focus:enabled {
			background-color: var(--clr-bg-1-muted);
			& .icon {
				color: var(--clr-scale-ntrl-40);
			}
		}
		&:disabled {
			background-color: var(--clr-bg-2);
			color: var(--clr-scale-ntrl-50);
		}
		& .icon {
			display: flex;
			color: var(--clr-scale-ntrl-50);
		}
		& .label {
			height: 16px;
			text-overflow: ellipsis;
			overflow-x: hidden;
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
