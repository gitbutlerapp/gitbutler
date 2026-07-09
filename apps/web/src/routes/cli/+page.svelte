<script lang="ts">
	import ComparisonTable from "./sections/ComparisonTable.svelte";
	import CtaSection from "./sections/CtaSection.svelte";
	import Features from "./sections/Features.svelte";
	import Hero from "./sections/Hero.svelte";
	import OptimizedForAgents from "./sections/OptimizedForAgents.svelte";
	import Footer from "$lib/components/marketing/Footer.svelte";
</script>

<svelte:head>
	<title>GitButler | CLI</title>
</svelte:head>

<Hero currentPage="cli">
	{#snippet descriptionContent()}
		Ein intuitiveres Git-Terminal-Erlebnis. Gestapelte Branches, unbegrenztes Rückgängigmachen und
		parallele Branches funktionieren genau so, wie du es erwartest. Perfekt für Teams, einzelne
		Entwickler oder KI-gestütztes Coding.
		<br /><br />
		Funktioniert mit jedem Git-Repo. Fügt sich nahtlos in deinen Workflow ein.
	{/snippet}
</Hero>
<Features />
<ComparisonTable />
<OptimizedForAgents />
<CtaSection />
<Footer />

<style>
</style>
