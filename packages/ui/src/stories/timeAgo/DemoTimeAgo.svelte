<script lang="ts">
	import TimeAgo from '$lib/timeAgo/TimeAgo.svelte';

	interface Props {
		date: number;
		addSuffix?: boolean;
	}

	const { date, addSuffix }: Props = $props();
</script>

<TimeAgo date={new Date(date)} {addSuffix}></TimeAgo>
