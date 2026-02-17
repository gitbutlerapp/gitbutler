<script module lang="ts">
	import SimpleCommitRowSkeleton from "$components/SimpleCommitRowSkeleton.svelte";
	import { defineMeta } from "@storybook/addon-svelte-csf";

	const { Story } = defineMeta({
		title: "Components / SimpleCommitRowSkeleton",
		component: SimpleCommitRowSkeleton,
		args: {
			onlyContent: false,
		},
	});
</script>

<Story name="Default">
	{#snippet template(args)}
		<SimpleCommitRowSkeleton onlyContent={args.onlyContent} />
	{/snippet}
</Story>

<Story name="Only Content">
	{#snippet template()}
		<SimpleCommitRowSkeleton onlyContent />
	{/snippet}
</Story>

<Story name="Multiple">
	{#snippet template()}
		<div>
			<SimpleCommitRowSkeleton />
			<SimpleCommitRowSkeleton />
			<SimpleCommitRowSkeleton />
		</div>
	{/snippet}
</Story>
