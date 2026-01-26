<script module lang="ts">
	import HunkDiffSkeleton from '$components/hunkDiff/HunkDiffSkeleton.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Code / HunkDiffSkeleton',
		component: HunkDiffSkeleton,
		args: {
			lineCount: 8
		}
	});
</script>

<Story name="Playground">
	{#snippet template(args)}
		<div class="wrap">
			<HunkDiffSkeleton {...args} />
		</div>
	{/snippet}
</Story>

<style>
	.wrap {
		display: flex;
		width: 600px;
		padding: 20px;
	}
</style>
