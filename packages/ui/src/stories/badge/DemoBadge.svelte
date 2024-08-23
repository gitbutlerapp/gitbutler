<script lang="ts">
	import Badge, { type BadgeProps } from '$lib/Badge.svelte';

	const props: BadgeProps = $props();
</script>

<Badge {...props} />
