<script lang="ts">
	import Tooltip from "$lib/components/Tooltip.svelte";
	import { createTimeAgoStore, getAbsoluteTimestamp } from "$lib/utils/timeAgo";

	interface Props {
		date?: Date;
		addSuffix?: boolean;
		showTooltip?: boolean;
		capitalize?: boolean;
	}

	const { date, addSuffix, showTooltip = true, capitalize = false }: Props = $props();
	const store = $derived(createTimeAgoStore(date, addSuffix));
	const absoluteTime = $derived(date ? getAbsoluteTimestamp(date) : "");

	function formatText(value: string | undefined): string {
		if (!value) return "";
		if (!capitalize) return value;
		return `${value[0].toUpperCase()}${value.slice(1)}`;
	}
</script>

{#if store}
	{#if showTooltip && date}
		<Tooltip text={absoluteTime}>
			<span>{formatText($store)}</span>
		</Tooltip>
	{:else}
		{formatText($store)}
	{/if}
{/if}
