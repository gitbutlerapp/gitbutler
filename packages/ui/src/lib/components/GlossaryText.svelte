<script lang="ts">
	import GitTerm from "$components/GitTerm.svelte";
	import { allGitTermKeys, splitGlossaryText, type GitTermKey } from "$lib/utils/gitGlossary";

	interface Props {
		text: string;
		terms?: readonly GitTermKey[];
		maxWidth?: number;
	}

	const { text, terms = allGitTermKeys, maxWidth }: Props = $props();

	const segments = $derived(splitGlossaryText(text, terms));
</script>

<span class="glossary-text">
	{#each segments as segment}
		{#if segment.type === "term"}
			<GitTerm term={segment.term} text={segment.text} {maxWidth} />
		{:else}
			{segment.text}
		{/if}
	{/each}
</span>

<style lang="postcss">
	.glossary-text {
		display: contents;
	}
</style>
