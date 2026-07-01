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
		A more intuitive Git terminal experience. Stacked branches, unlimited undo, and parallel
		branches work exactly how you'd expect. Perfect for teams, solo developers or AI-assisted
		coding.
		<br /><br />
		Works with any Git repo. Seamlessly fits your workflow.
	{/snippet}
</Hero>
<Features />
<ComparisonTable />
<OptimizedForAgents />
<CtaSection />
<Footer />

<style>
</style>
