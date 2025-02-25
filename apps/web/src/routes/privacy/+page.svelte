<script lang="ts">
	import * as jsonLinks from '$home/lib/data/links.json';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';

	onMount(() => {
		goto(jsonLinks.legal.privacyPolicy.url);
	});
</script>

<svelte:window
	onload={() => {
		location.href = jsonLinks.legal.privacyPolicy.url;
	}}
/>
