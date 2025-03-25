<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import type iconsJson from '@gitbutler/ui/data/icons.json';

	interface Props {
		iconName: keyof typeof iconsJson;
		isActivated?: boolean;
		tooltip: string;
		onClick: () => void;
	}

	const { iconName, isActivated, tooltip, onClick }: Props = $props();
</script>

<Tooltip text={tooltip} delay={1000}>
	<button type="button" class="formatting-button" class:actiated={isActivated} onclick={onClick}>
		<Icon name={iconName} />
	</button>
</Tooltip>

<style lang="postcss">
	.formatting-button {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: var(--size-tag);
		border-radius: var(--radius-s);
		color: var(--clr-theme-ntrl-on-element);
		background-color: var(--clr-theme-ntrl-element);
		overflow: hidden;
		transition: background-color var(--transition-fast);

		&::after {
			display: none;
			position: absolute;
			content: '';
			width: 6px;
			height: 6px;
			top: 2px;
			right: 2px;
			border-radius: var(--radius-s);
			background-color: var(--clr-theme-pop-element);
			outline: 2px solid var(--clr-theme-ntrl-element-hover);
		}

		&:hover {
			background-color: var(--clr-theme-ntrl-element-hover);
		}

		&.actiated {
			background-color: var(--clr-theme-ntrl-element-hover);

			&::after {
				display: block;
			}
		}
	}
</style>
