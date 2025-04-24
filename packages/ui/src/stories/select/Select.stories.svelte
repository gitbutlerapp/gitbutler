<script module lang="ts">
	import Button from '$lib/Button.svelte';
	import Select from '$lib/select/Select.svelte';
	import SelectItem from '$lib/select/SelectItem.svelte';
	import {
		type Args,
		defineMeta,
		setTemplate,
		type StoryContext
	} from '@storybook/addon-svelte-csf';

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
	setTemplate(templateDefault);
	setTemplate(templateCustomButton);
</script>

{#snippet templateDefault({ ...args }: Args<typeof Story>, _context: StoryContext<typeof Story>)}
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

{#snippet templateCustomButton(
	{ ...args }: Args<typeof Story>,
	_context: StoryContext<typeof Story>
)}
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

<Story name="Default" />
<Story name="Custom button" />

<style>
	.wrap {
		display: flex;
		justify-content: center;
		align-items: center;
		height: 600px;
		width: 100%;
		background: var(--clr-bg-2);
		border-radius: var(--radius-ml);
	}
</style>
