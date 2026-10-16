<script lang="ts">
	import Icon from "$components/Icon.svelte";
	import Tooltip from "$components/Tooltip.svelte";
	import { type IconName } from "$lib/icons/names";

	interface Props {
		single: boolean;
		origin?: boolean;
		size?: number;
	}

	const { single, origin, size }: Props = $props();

	function getIcon(): IconName {
		if (origin) {
			return "home";
		}

		if (single) {
			return "branch";
		}

		return "link";
	}
</script>

<Tooltip text={single ? "Single branch" : "Multiple branches"}>
	<div class="stack-icon">
		<Icon name={getIcon()} {size} />
	</div>
</Tooltip>

<style>
	.stack-icon {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		color: var(--text-2);
	}
</style>
