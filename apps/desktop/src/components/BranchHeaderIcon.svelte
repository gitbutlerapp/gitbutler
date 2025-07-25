<script lang="ts">
	import { Icon } from '@gitbutler/ui';
	import type iconsJson from '@gitbutler/ui/data/icons.json';

	interface Props {
		iconName: keyof typeof iconsJson;
		color: string;
		lineTop?: boolean;
		lineBottom?: boolean;
		isDashed?: boolean;
	}

	const { iconName, color }: Props = $props();
</script>

<div class="stack__status--icon" style:--bg-color={color}>
	<Icon name={iconName} />
</div>

<style>
	.stack__status--icon {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: 20px;
		height: 20px;
		border-radius: var(--radius-m);
		background-color: var(--bg-color);
		color: #fff;
	}
</style>
