<script lang="ts">
	import AiFeatures from "$home/sections/AiFeatures.svelte";
	import BlogHighlights from "$home/sections/BlogHighlights.svelte";
	import Changelog from "$home/sections/Changelog.svelte";
	import FeatureUpdates from "$home/sections/FeatureUpdates.svelte";
	import Hero from "$home/sections/Hero.svelte";
	import MainFeatures from "$home/sections/MainFeatures.svelte";
	import SocialQuotes from "$home/sections/SocialQuotes.svelte";
	import Footer from "$lib/components/marketing/Footer.svelte";
	import { getValidReleases } from "$lib/types/releases";
	import { onMount } from "svelte";

	let releases: any[] = $state([]);

	onMount(() => {
		// Fetch latest 10 releases for changelog
		fetch("https://app.gitbutler.com/api/downloads?limit=10&channel=release")
			.then((response) => response.json())
			.then((data) => {
				releases = getValidReleases(data);
			})
			.catch((error) => {
				console.error("Failed to fetch releases for changelog:", error);
			});
	});
</script>

<Hero>
	{#snippet descriptionContent()}
		GitButler is the Git-backed change management tool for modern, AI&nbsp;coding workflows.
		Parallel and stacked branches, unlimited undo, agent integrations, and more. It's Git, refined.
	{/snippet}
</Hero>
<MainFeatures />
<AiFeatures />
<FeatureUpdates />
<SocialQuotes />
<Changelog {releases} />
<BlogHighlights />
<Footer />
