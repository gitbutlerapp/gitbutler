<script module lang="ts">
	import Badge from '$lib/Badge.svelte';
	import iconsJson from '$lib/data/icons.json';
	import {
		type Args,
		defineMeta,
		setTemplate,
		type StoryContext
	} from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Basic / Badge',
		component: Badge,
		args: {
			children: 'Badge' as any,
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

<script lang="ts">
	setTemplate(template);
</script>

{#snippet template({ ...args }: Args<typeof Story>, _context: StoryContext<typeof Story>)}
	<Badge
		style={args.style}
		kind={args.kind}
		icon={args.icon}
		size={args.size}
		reversedDirection={args.reversedDirection}
	>
		{args.children}
	</Badge>
{/snippet}

<Story name="Playground" />
