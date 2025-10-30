<script lang="ts">
	import { goto } from '$app/navigation';

	// Immediately redirect to /nightly
	$effect(() => {
		goto('/nightly', { replaceState: true });
	});
</script>

<!-- This page will immediately redirect to /nightly -->
