<script lang="ts">
	import Tooltip from '$lib/components/Tooltip.svelte';
	import { createTimestampStore, getAbsoluteTimestamp } from '$lib/utils/timeAgo';

	interface Props {
		date?: string | Date;
		showTooltip?: boolean;
		showSeconds?: boolean;
	}

	const parsedDate = $derived.by(() => {
		if (typeof date === 'string') {
			if (date.endsWith('Z')) return new Date(date);
			return new Date(date + 'Z');
		}
		return date;
	});
	const { date, showTooltip = true }: Props = $props();
	const store = $derived(createTimestampStore(parsedDate));
	const absoluteTime = $derived(getAbsoluteTimestamp(parsedDate));
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
