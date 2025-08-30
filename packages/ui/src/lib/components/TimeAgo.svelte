<script lang="ts">
	import Tooltip from '$lib/components/Tooltip.svelte';
	import { createTimeAgoStore, getAbsoluteTimestamp } from '$lib/utils/timeAgo';

	interface Props {
		date?: Date;
		addSuffix?: boolean;
		showTooltip?: boolean;
	}

	const { date, addSuffix, showTooltip = true }: Props = $props();
	const store = $derived(createTimeAgoStore(date, addSuffix));
	const absoluteTime = $derived(date ? getAbsoluteTimestamp(date) : '');
</script>

{#if store}
	{#if showTooltip && date}
		<Tooltip text={absoluteTime}>
			<span>{$store}</span>
		</Tooltip>
	{:else}
		{$store}
	{/if}
{/if}
