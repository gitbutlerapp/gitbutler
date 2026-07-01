<script module lang="ts">
	import HunkDiffSkeleton from "$components/hunkDiff/HunkDiffSkeleton.svelte";
	import { defineMeta } from "@storybook/addon-svelte-csf";

	const { Story } = defineMeta({
		title: "Code / HunkDiffSkeleton",
		component: HunkDiffSkeleton,
	});
</script>

<Story name="Playground">
	{#snippet template()}
		<div class="wrap">
			<HunkDiffSkeleton />
		</div>
	{/snippet}
</Story>

<style>
	.wrap {
		display: flex;
		max-width: 600px;
	}
</style>
