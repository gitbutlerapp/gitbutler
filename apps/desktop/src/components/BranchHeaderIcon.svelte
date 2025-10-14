<script lang="ts">
	import { Icon } from '@gitbutler/ui';
	import type iconsJson from '@gitbutler/ui/data/icons.json';

	interface Props {
		iconName: keyof typeof iconsJson;
		color: string;
		small?: boolean;
		large?: boolean;
	}

	const { iconName, color, small, large }: Props = $props();
</script>

<div class="stack__status--icon" style:--bg-color={color} class:small class:large>
	<Icon name={iconName} />
</div>

<style>
	.stack__status--icon {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		background-color: var(--bg-color);
		color: #fff;

		&:not(.small):not(.large) {
			width: 20px;
			height: 20px;
			border-radius: var(--radius-m);
		}

		&.small {
			width: 18px;
			height: 18px;
			border-radius: var(--radius-m);
		}

		&.large {
			width: var(--size-button);
			height: var(--size-button);
			border-radius: var(--radius-ml);
		}
	}
</style>
