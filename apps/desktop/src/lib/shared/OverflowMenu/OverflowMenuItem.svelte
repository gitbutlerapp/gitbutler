<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import type iconsJson from '@gitbutler/ui/data/icons.json';

	interface Props {
		el?: HTMLButtonElement;
		icon: keyof typeof iconsJson;
		tooltip: string;
		thin?: boolean;
		onclick: (e: MouseEvent) => void;
	}

	let { el = $bindable(), icon, tooltip, thin, onclick }: Props = $props();
</script>

<Tooltip text={tooltip} position="top" delay={200}>
	<button
		bind:this={el}
		data-clickable="true"
		class="overflow-actions-btn focus-state"
		class:thin
		onclick={(e) => {
			e.preventDefault();
			e.stopPropagation();
			onclick(e);
		}}
	>
		<div class="overflow-actions-btn__icon">
			<Icon name={icon} />
		</div>
	</button>
</Tooltip>

<style lang="postcss">
	.overflow-actions-btn {
		padding: 3px 5px;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--clr-text-1);

		background-color: var(--clr-bg-1);
		border-left: 1px solid var(--clr-border-2);
		border-top: 1px solid var(--clr-border-2);
		border-bottom: 1px solid var(--clr-border-2);
		transition:
			background-color var(--transition-fast),
			opacity var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1-muted);

			.overflow-actions-btn__icon {
				opacity: 1;
			}
		}
	}

	.overflow-actions-btn.thin {
		padding: 0px 4px;
		background-color: red;
	}

	.overflow-actions-btn__icon {
		pointer-events: none;
		display: flex;
		opacity: 0.5;
	}
</style>
