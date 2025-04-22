<script module lang="ts">
	import Button from '$lib/Button.svelte';
	import {
		type Args,
		defineMeta,
		setTemplate,
		type StoryContext
	} from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / Button',
		args: {
			style: 'neutral',
			kind: 'solid',
			outline: false
		},
		argTypes: {
			style: {
				options: ['neutral', 'pop', 'success', 'error', 'warning', 'purple'],
				control: {
					type: 'select'
				}
			},
			kind: {
				options: ['solid', 'outline', 'ghost'],
				control: {
					type: 'select'
				}
			}
		}
	});
</script>

<script lang="ts">
	setTemplate(template);
</script>

{#snippet template({ ...args }: Args<typeof Story>, _context: StoryContext<typeof Story>)}
	<div class="drodown-btn">
		<Button style={args.style} kind={args.kind} dropdownChild>Dropdown</Button>
		<Button style={args.style} kind={args.kind} icon="chevron-down-small" dropdownChild />
	</div>
{/snippet}

<Story name="Dropdown" />

<style>
	.drodown-btn {
		display: flex;
		max-width: 200px;
	}
</style>
