<script module lang="ts">
	import InfoButton from '$lib/InfoButton.svelte';
	import {
		type Args,
		defineMeta,
		setTemplate,
		type StoryContext
	} from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Overlays / InfoButton',
		component: InfoButton,
		args: {
			title: 'The title of the InfoButton',
			children: 'This is the content of the InfoButton' as any,
			size: 'medium',
			icon: 'info'
		},
		argTypes: {
			size: {
				options: ['small', 'medium', 'large'],
				control: {
					type: 'select'
				}
			},
			title: {
				control: {
					type: 'text'
				}
			},
			children: {
				control: {
					type: 'text'
				}
			}
		}
	});
</script>

<script lang="ts">
	setTemplate(template);
</script>

{#snippet template({ ...args }: Args<typeof Story>, _context: StoryContext<typeof Story>)}
	<div class="text-wrap text-14">
		This is an example of only text with an InfoButton.
		<InfoButton size={args.size} title={args.title} icon={args.icon}>
			{args.children}
		</InfoButton>
	</div>
{/snippet}

<Story name="Playground" />

<style>
	.text-wrap {
		width: 260px;
		padding: 20px;
	}
</style>
