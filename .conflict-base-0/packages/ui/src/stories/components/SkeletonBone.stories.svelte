<script module lang="ts">
	import SkeletonBone from '$components/SkeletonBone.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Components / SkeletonBone',
		component: SkeletonBone,
		args: {
			width: '100%',
			height: '1rem',
			radius: 'var(--radius-ml)',
			color: 'var(--clr-text-3)',
			opacity: 0.14
		}
	});
</script>

<Story name="Playground">
	{#snippet template(args)}
		<div style="padding: 20px; max-width: 400px;">
			<SkeletonBone
				width={args.width}
				height={args.height}
				radius={args.radius}
				color={args.color}
				opacity={args.opacity}
			/>
		</div>
	{/snippet}
</Story>

<Story name="Text Lines">
	{#snippet template()}
		<div style="padding: 20px; max-width: 600px; display: flex; flex-direction: column; gap: 8px;">
			<SkeletonBone width="100%" height="1rem" />
			<SkeletonBone width="90%" height="1rem" />
			<SkeletonBone width="95%" height="1rem" />
			<SkeletonBone width="70%" height="1rem" />
		</div>
	{/snippet}
</Story>

<Story name="Button">
	{#snippet template()}
		<div style="padding: 20px;">
			<SkeletonBone width="6rem" height="var(--size-button)" radius="var(--radius-btn)" />
		</div>
	{/snippet}
</Story>

<Story name="Avatar">
	{#snippet template()}
		<div style="padding: 20px;">
			<SkeletonBone width="2.5rem" height="2.5rem" radius="50%" />
		</div>
	{/snippet}
</Story>

<Story name="Card">
	{#snippet template()}
		<div style="padding: 20px; max-width: 400px;">
			<SkeletonBone width="100%" height="10rem" radius="var(--radius-m)" />
		</div>
	{/snippet}
</Story>

<Story name="Custom Color">
	{#snippet template()}
		<div style="padding: 20px; max-width: 400px; display: flex; flex-direction: column; gap: 12px;">
			<SkeletonBone
				width="80%"
				height="3rem"
				color="var(--clr-bg-3)"
				opacity={0.4}
				radius="var(--radius-ml)"
			/>
			<SkeletonBone
				width="100%"
				height="2rem"
				color="var(--clr-theme-pop-element)"
				opacity={0.2}
				radius="var(--radius-btn)"
			/>
		</div>
	{/snippet}
</Story>
