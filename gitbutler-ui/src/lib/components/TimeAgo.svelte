<script lang="ts">
	import { createTimeAgoStore } from '$lib/utils/timeAgo';

	export let date: Date | undefined;
	export let addSuffix = true;
	$: store = createTimeAgoStore(date, addSuffix);
</script>

{#if store}
	{$store}
{/if}
