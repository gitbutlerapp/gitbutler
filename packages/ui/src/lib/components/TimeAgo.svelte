<script lang="ts">
	import { createTimeAgoStore } from '$lib/utils/timeAgo';

	interface Props {
		date?: Date;
		addSuffix?: boolean;
	}

	const { date, addSuffix }: Props = $props();
	const store = $derived(createTimeAgoStore(date, addSuffix));
</script>

{#if store}
	{$store}
{/if}
