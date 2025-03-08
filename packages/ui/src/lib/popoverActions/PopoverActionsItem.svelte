<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import type iconsJson from '@gitbutler/ui/data/icons.json';

	interface Props {
		el?: HTMLButtonElement;
		icon: keyof typeof iconsJson;
		tooltip: string;
		thin?: boolean;
		activated?: boolean;
		disabled?: boolean;
		onclick: (e: MouseEvent) => void;
	}

	let { el = $bindable(), icon, tooltip, thin, activated, onclick, disabled }: Props = $props();
</script>

<Tooltip disabled={activated || disabled} text={tooltip} position="top" delay={200}>
	<button
		type="button"
		bind:this={el}
		data-clickable="true"
		class="overflow-actions-btn focus-state"
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
		<div class="overflow-actions-btn__icon">
			<Icon name={icon} />
		</div>
	</button>
</Tooltip>

<style lang="postcss">
	.overflow-actions-btn {
		--label-clr: var(--clr-btn-ntrl-outline-text);
		--icon-opacity: var(--opacity-btn-icon-outline);
		--btn-bg: var(--clr-bg-1);
		--opacity-btn-bg: 0;

		color: var(--label-clr);
		background: color-mix(
			in srgb,
			var(--btn-bg, transparent),
			var(--clr-btn-ntrl-outline-bg) calc(var(--opacity-btn-bg, 0) * 100%)
		);
		border: 1px solid var(--clr-border-2);
		border-right: none;

		padding: 3px 5px;
		display: flex;
		align-items: center;
		justify-content: center;
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
		pointer-events: none;
		display: flex;
		opacity: var(--icon-opacity);
	}
</style>
