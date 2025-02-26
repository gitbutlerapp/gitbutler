<script lang="ts">
	import * as jsonLinks from '$home/data/links.json';
	import { onMount } from 'svelte';

	onMount(() => {
		window.location = jsonLinks.legal.privacyPolicy.url;
	});
</script>

<svelte:window
	onload={() => {
		location.href = jsonLinks.legal.privacyPolicy.url;
	}}
/>
