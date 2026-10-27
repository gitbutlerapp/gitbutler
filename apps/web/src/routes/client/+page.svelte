<script lang="ts">
	import { page } from "$app/state";
	import HomePage from "$home/HomePage.svelte";
	import { pageTitle } from "$lib/meta/opengraph";
</script>

<svelte:head>
	<title>{pageTitle(page.url.pathname)}</title>
</svelte:head>

<HomePage />
