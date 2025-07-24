<script module lang="ts">
	import InfoButton from '$lib/InfoButton.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Overlays / InfoButton',
		component: InfoButton,
		args: {
			title: 'The title of the InfoButton',
			children: 'This is the content of the InfoButton' as any,
			size: 'medium'
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
</script>

<Story name="default">
	{#snippet template(args)}
		<div class="text-wrap text-14">
			This is an example of only text with an InfoButton.
			<InfoButton size={args.size} title={args.title} icon={args.icon}>
				{args.children}
			</InfoButton>
		</div>
	{/snippet}
</Story>

<Story name="Playground" />

<style>
	.text-wrap {
		width: 260px;
		padding: 20px;
	}
</style>
