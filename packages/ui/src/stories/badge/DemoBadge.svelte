<script lang="ts">
	import Badge, { type Props as BadgeProps } from '$lib/Badge.svelte';

	const props: BadgeProps = $props();
</script>

<Badge {...props} />
