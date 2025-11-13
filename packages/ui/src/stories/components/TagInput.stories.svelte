<script module lang="ts">
	import TagInput from '$components/TagInput.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import type { Tag } from '$components/TagInput.svelte';

	const { Story } = defineMeta({
		title: 'Inputs / TagInput',
		component: TagInput,
		args: {
			placeholder: 'Add tags…',
			disabled: false,
			readonly: false,
			autofocus: false,
			wide: false,
			label: '',
			helperText: 'Press comma or space to add a tag',
			error: '',
			maxTags: undefined,
			width: undefined
		},
		argTypes: {
			maxTags: {
				control: { type: 'number' }
			},
			width: {
				control: { type: 'number' }
			}
		}
	});
</script>

<script lang="ts">
	let basicTags = $state<Tag[]>([]);
</script>

<Story name="Default">
	{#snippet template(args)}
		<div class="wrap">
			<TagInput
				bind:tags={basicTags}
				placeholder={args.placeholder}
				disabled={args.disabled}
				readonly={args.readonly}
				wide={args.wide}
				label={args.label}
				helperText={args.helperText}
				error={args.error}
				maxTags={args.maxTags}
				width={args.width}
			/>
		</div>
	{/snippet}
</Story>

<style>
	.wrap {
		max-width: 400px;
		padding: 16px;
	}
</style>
