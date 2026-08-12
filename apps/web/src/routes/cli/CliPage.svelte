<script lang="ts">
	import ComparisonTable from "./sections/ComparisonTable.svelte";
	import CtaSection from "./sections/CtaSection.svelte";
	import Features from "./sections/Features.svelte";
	import Hero from "./sections/Hero.svelte";
	import OptimizedForAgents from "./sections/OptimizedForAgents.svelte";
	import Footer from "$lib/components/marketing/Footer.svelte";
</script>

<svelte:head>
	<title>GitButler</title>
</svelte:head>

<Hero currentPage="cli">
	{#snippet descriptionContent()}
		Agents are 60% faster using GitButler than vanilla Git, with 80% fewer tool calls. Works with
		any Git repo. Seamlessly fits your workflow. See the benchmark at
		<a href="https://vcbench.dev/" target="_blank" rel="noopener noreferrer">vcbench.dev</a>.
	{/snippet}
</Hero>
<Features />
<ComparisonTable />
<OptimizedForAgents />
<CtaSection />
<Footer />

<style lang="postcss">
	a {
		text-decoration: underline;
	}
</style>
