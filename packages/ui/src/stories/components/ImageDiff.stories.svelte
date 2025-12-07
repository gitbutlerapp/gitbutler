<script module lang="ts">
	import ImageDiff from '$components/ImageDiff.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'File Diff / ImageDiff',
		component: ImageDiff as any,
		args: {
			beforeImageUrl: null,
			afterImageUrl: null,
			fileName: 'example.png',
			isLoading: false
		},
		argTypes: {
			beforeImageUrl: {
				control: { type: 'text' }
			},
			afterImageUrl: {
				control: { type: 'text' }
			},
			fileName: {
				control: { type: 'text' }
			},
			isLoading: {
				control: { type: 'boolean' }
			}
		}
	});

	// Sample image data URLs for demonstration
	const sampleImageBefore =
		'data:image/svg+xml,%3Csvg xmlns="http://www.w3.org/2000/svg" width="300" height="200"%3E%3Crect width="300" height="200" fill="%23ff6b6b"/%3E%3Ctext x="150" y="100" text-anchor="middle" dominant-baseline="middle" font-size="24" fill="white"%3EBefore%3C/text%3E%3C/svg%3E';
	const sampleImageAfter =
		'data:image/svg+xml,%3Csvg xmlns="http://www.w3.org/2000/svg" width="500" height="400"%3E%3Crect width="500" height="400" fill="%234ecdc4"/%3E%3Ctext x="250" y="200" text-anchor="middle" dominant-baseline="middle" font-size="24" fill="white"%3EAfter%3C/text%3E%3C/svg%3E';
</script>

<Story name="Playground">
	{#snippet template(args)}
		<ImageDiff
			beforeImageUrl={args.beforeImageUrl}
			afterImageUrl={args.afterImageUrl}
			fileName={args.fileName}
			isLoading={args.isLoading}
		/>
	{/snippet}
</Story>

<Story
	name="Before and After"
	args={{ beforeImageUrl: sampleImageBefore, afterImageUrl: sampleImageAfter }}
>
	{#snippet template(args)}
		<ImageDiff
			beforeImageUrl={args.beforeImageUrl}
			afterImageUrl={args.afterImageUrl}
			fileName="logo.svg"
		/>
	{/snippet}
</Story>

<Story name="Only After (Addition)" args={{ afterImageUrl: sampleImageAfter }}>
	{#snippet template(args)}
		<ImageDiff afterImageUrl={args.afterImageUrl} fileName="new-icon.svg" />
	{/snippet}
</Story>

<Story name="Only Before (Deletion)" args={{ beforeImageUrl: sampleImageBefore }}>
	{#snippet template(args)}
		<ImageDiff beforeImageUrl={args.beforeImageUrl} fileName="old-icon.svg" />
	{/snippet}
</Story>

<Story name="Loading State" args={{ isLoading: true }}>
	{#snippet template(args)}
		<ImageDiff isLoading={args.isLoading} fileName="loading.png" />
	{/snippet}
</Story>
