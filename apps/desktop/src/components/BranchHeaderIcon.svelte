<script lang="ts">
	import { Icon } from '@gitbutler/ui';
	import type iconsJson from '@gitbutler/ui/data/icons.json';

	interface Props {
		iconName: keyof typeof iconsJson;
		color: string;
		small?: boolean;
	}

	const { iconName, color, small }: Props = $props();
</script>

<div class="stack__status--icon" style:--bg-color={color} class:small>
	<Icon name={iconName} />
</div>

<style>
	.stack__status--icon {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		border-radius: var(--radius-m);
		background-color: var(--bg-color);
		color: #fff;

		&:not(.small) {
			width: 20px;
			height: 20px;
		}

		&.small {
			width: 18px;
			height: 18px;
		}
	}
</style>
