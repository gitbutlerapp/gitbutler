<script module lang="ts">
	import Badge from '$components/Badge.svelte';
	import iconsJson from '$lib/data/icons.json';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Basic / Badge',
		component: Badge,
		args: {
			children,
			style: 'neutral',
			kind: 'solid',
			size: 'icon',
			icon: undefined,
			reversedDirection: false
		},
		argTypes: {
			children: {
				control: { type: 'text' }
			},
			style: {
				options: ['neutral', 'pop', 'success', 'warning', 'error', 'purple'],
				control: { type: 'select' }
			},
			kind: {
				options: ['solid', 'soft'],
				control: { type: 'select' }
			},
			size: {
				options: ['icon', 'tag'],
				control: { type: 'select' }
			},
			icon: {
				options: Object.keys(iconsJson),
				control: { type: 'select' }
			}
		}
	});
</script>

{#snippet children()}
	Badge
{/snippet}

<Story name="default">
	{#snippet template(args)}
		<Badge
			style={args.style}
			kind={args.kind}
			icon={args.icon}
			size={args.size}
			reversedDirection={args.reversedDirection}
			children={args.children}
		></Badge>
	{/snippet}
</Story>

<Story name="Playground" />
