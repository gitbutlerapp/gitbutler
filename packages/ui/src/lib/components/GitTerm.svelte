<script lang="ts">
	import Tooltip from "$components/Tooltip.svelte";
	import { getGitGlossaryExplanation, type GitTermKey } from "$lib/utils/gitGlossary";
	import type { Snippet } from "svelte";

	interface Props {
		term: GitTermKey;
		text?: string;
		maxWidth?: number;
		children?: Snippet;
	}

	const { term, text, maxWidth = 300, children }: Props = $props();
</script>

<Tooltip text={getGitGlossaryExplanation(term)} {maxWidth}>
	<span class="git-term">
		{#if children}
			{@render children()}
		{:else}
			{text ?? term}
		{/if}
	</span>
</Tooltip>

<style lang="postcss">
	.git-term {
		border-bottom: 1px dotted var(--text-3);
		text-underline-offset: 2px;
		cursor: help;
	}
</style>
