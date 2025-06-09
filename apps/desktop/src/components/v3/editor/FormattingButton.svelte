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
		display: flex;
		position: relative;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: var(--size-tag);
		overflow: hidden;
		border-radius: var(--radius-s);
		background-color: var(--clr-theme-ntrl-element);
		color: var(--clr-theme-ntrl-on-element);
		transition: background-color var(--transition-fast);

		&::after {
			display: none;
			position: absolute;
			top: 2px;
			right: 2px;
			width: 6px;
			height: 6px;
			border-radius: var(--radius-s);
			outline: 2px solid var(--clr-theme-ntrl-element-hover);
			background-color: var(--clr-theme-pop-element);
			content: '';
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
