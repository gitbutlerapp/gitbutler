<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import type iconsJson from '@gitbutler/ui/data/icons.json';

	interface Props {
		el?: HTMLButtonElement;
		icon: keyof typeof iconsJson;
		tooltip: string;
		onclick: () => void;
	}

	let { el = $bindable(), icon, tooltip, onclick }: Props = $props();
</script>

<Tooltip text={tooltip} position="top" delay={200}>
	<button
		bind:this={el}
		data-clickable="true"
		class="overflow-actions-btn"
		onclick={(e) => {
			e.preventDefault();
			e.stopPropagation();

			onclick();
		}}
	>
		<Icon name={icon} />
	</button>
</Tooltip>

<style lang="postcss">
	.overflow-actions-btn {
		padding: 3px 4px;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--clr-text-1);
		opacity: 0.5;
		border-left: 1px solid var(--clr-border-2);

		&:hover {
			background-color: var(--clr-bg-1-muted);
			opacity: 0.8;
		}
	}
</style>
