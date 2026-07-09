<script lang="ts">
	import AiFeatures from "$home/sections/AiFeatures.svelte";
	import BlogHighlights from "$home/sections/BlogHighlights.svelte";
	import Changelog from "$home/sections/Changelog.svelte";
	import FeatureUpdates from "$home/sections/FeatureUpdates.svelte";
	import Hero from "$home/sections/Hero.svelte";
	import MainFeatures from "$home/sections/MainFeatures.svelte";
	import SocialQuotes from "$home/sections/SocialQuotes.svelte";
	import Footer from "$lib/components/marketing/Footer.svelte";

	interface Props {
		releases?: any[];
	}

	const { releases = [] }: Props = $props();
</script>

<Hero>
	{#snippet descriptionContent()}
		GitButler ist das Git-basierte Änderungsmanagement-Tool für moderne KI&nbsp;Coding-Workflows.
		Parallele und gestapelte Branches, unbegrenztes Rückgängigmachen, Agent-Integrationen und mehr. Das ist Git, verfeinert.
	{/snippet}
</Hero>
<MainFeatures />
<AiFeatures />
<FeatureUpdates />
<SocialQuotes />
<Changelog {releases} />
<BlogHighlights />
<Footer />
