<script module lang="ts">
	import Button from '$lib/Button.svelte';
	import Select from '$lib/select/Select.svelte';
	import SelectItem from '$lib/select/SelectItem.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / Select',
		args: {
			options: [
				{ value: '1', label: 'Option 1' },
				{ value: '2', label: 'Option 2' },
				{ value: '3', label: 'Option 3' },
				{ value: '4', label: 'Option 4' },
				{ value: '5', label: 'Option 5' }
			]
		},
		argTypes: {}
	});

	let selectedItem = $state<string>('1');
</script>

<script lang="ts">
</script>

<Story name="Default">
	{#snippet template(args)}
		<div class="wrap">
			<Select
				searchable
				options={args.options}
				value={selectedItem}
				onselect={(value: string) => {
					selectedItem = value;
				}}
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={highlighted} {highlighted}>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>
		</div>
	{/snippet}
</Story>

<Story name="Custom button">
	{#snippet template(args)}
		<div class="wrap">
			<Select
				searchable
				options={args.options}
				value={selectedItem}
				onselect={(value: string) => {
					selectedItem = value;
				}}
				customWidth={120}
				popupAlign="center"
			>
				{#snippet customSelectButton()}
					<Button kind="outline" icon="select-chevron" size="tag">
						{args.options.find(
							(option: { value: string; label: string }) => option.value === selectedItem
						)?.label}
					</Button>
				{/snippet}
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={highlighted} {highlighted}>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>
		</div>
	{/snippet}
</Story>

<style>
	.wrap {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 600px;
		border-radius: var(--radius-ml);
		background: var(--clr-bg-2);
	}
</style>
