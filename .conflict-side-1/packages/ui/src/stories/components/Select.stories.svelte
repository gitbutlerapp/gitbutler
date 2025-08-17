<script module lang="ts">
	import Button from '$components/Button.svelte';
	import Icon from '$components/Icon.svelte';
	import Select from '$components/select/Select.svelte';
	import SelectItem from '$components/select/SelectItem.svelte';
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
			],
			optionsWithIcons: [
				{ value: 'js', label: 'JavaScript', icon: 'ai' },
				{ value: 'ts', label: 'TypeScript', icon: 'branch-small' },
				{ value: 'py', label: 'Python', icon: 'idea' },
				{ value: 'rust', label: 'Rust', icon: 'docs' },
				{ value: 'go', label: 'Go', icon: 'minus-small' }
			],
			optionsWithEmojis: [
				{ value: 'happy', label: 'Happy', emoji: '😊' },
				{ value: 'sad', label: 'Sad', emoji: '😢' },
				{ value: 'excited', label: 'Excited', emoji: '🎉' },
				{ value: 'cool', label: 'Cool', emoji: '😎' },
				{ value: 'love', label: 'Love', emoji: '❤️' }
			],
			longOptions: [
				{
					value: '1',
					label: 'This is a very long option that tests text overflow behavior in select items'
				},
				{
					value: '2',
					label:
						'Another extremely long option with lots of text to see how the component handles lengthy content and text wrapping scenarios'
				},
				{
					value: '3',
					label:
						'Super duper ultra mega long option name that goes on and on and on to test the ellipsis functionality'
				},
				{
					value: '4',
					label:
						'A moderately long option that is not as long as the others but still quite lengthy indeed'
				},
				{ value: '5', label: 'Short option' },
				{ value: '6', label: 'Medium length option text here' },
				{
					value: '7',
					label:
						'This option has a very long name that includes multiple words and should demonstrate how the select component handles text truncation with ellipsis when the content exceeds the available width'
				},
				{ value: '8', label: 'Final long option to complete the test suite for overflow behavior' }
			],
			optionsWithSeparators: [
				{ value: 'new', label: '📄 New Document' },
				{ value: 'open', label: '📂 Open Document' },
				{ value: 'recent', label: '🕐 Recent Documents' },
				{ separator: true },
				{ value: 'save', label: '💾 Save' },
				{ value: 'save-as', label: '💾 Save As...' },
				{ value: 'export', label: '📤 Export' },
				{ separator: true },
				{ value: 'cut', label: '✂️ Cut' },
				{ value: 'copy', label: '📋 Copy' },
				{ value: 'paste', label: '📋 Paste' },
				{ separator: true },
				{ value: 'settings', label: '⚙️ Settings' },
				{ value: 'help', label: '❓ Help' },
				{ value: 'about', label: 'ℹ️ About' }
			]
		},
		argTypes: {}
	});

	let selectedItem = $state<string>('1');
	let selectedWithIcon = $state<string>('js');
	let selectedWithEmoji = $state<string>('happy');
	let selectedLongOption = $state<string>('1');
	let selectedWithSeparators = $state<string>('new');
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

<Story name="With Icons">
	{#snippet template(args)}
		<div class="wrap">
			<Select
				searchable
				options={args.optionsWithIcons}
				value={selectedWithIcon}
				onselect={(value: string) => {
					selectedWithIcon = value;
				}}
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={highlighted} {highlighted}>
						{#snippet iconSnippet()}
							<Icon name={item.icon} />
						{/snippet}
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>
		</div>
	{/snippet}
</Story>

<Story name="With Emojis">
	{#snippet template(args)}
		<div class="wrap">
			<Select
				searchable
				options={args.optionsWithEmojis}
				value={selectedWithEmoji}
				onselect={(value: string) => {
					selectedWithEmoji = value;
				}}
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={highlighted} {highlighted}>
						{#snippet iconSnippet()}
							<span class="emoji">{item.emoji}</span>
						{/snippet}
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>
		</div>
	{/snippet}
</Story>

<Story name="Mixed Icons">
	{#snippet template(_args)}
		<div class="wrap">
			<Select
				searchable
				options={[
					{ value: 'builtin', label: 'Built-in Icon' },
					{ value: 'emoji', label: 'Custom Emoji', emoji: '🎨' },
					{ value: 'custom', label: 'Custom Component' }
				]}
				value={selectedWithIcon}
				onselect={(value: string) => {
					selectedWithIcon = value;
				}}
			>
				{#snippet itemSnippet({ item, highlighted })}
					{#if item.value === 'builtin'}
						<SelectItem selected={highlighted} {highlighted} icon="ai">
							{item.label}
						</SelectItem>
					{:else if item.value === 'emoji'}
						<SelectItem selected={highlighted} {highlighted}>
							{#snippet iconSnippet()}
								<span class="emoji">{item.emoji}</span>
							{/snippet}
							{item.label}
						</SelectItem>
					{:else}
						<SelectItem selected={highlighted} {highlighted}>
							{#snippet iconSnippet()}
								<div class="custom-component">
									<Icon name="plus-small" />
									<Icon name="minus-small" />
								</div>
							{/snippet}
							{item.label}
						</SelectItem>
					{/if}
				{/snippet}
			</Select>
		</div>
	{/snippet}
</Story>

<Story name="Long Options">
	{#snippet template(args)}
		<div class="wrap">
			<Select
				searchable
				options={args.longOptions}
				value={selectedLongOption}
				onselect={(value: string) => {
					selectedLongOption = value;
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

<Story name="With Separators">
	{#snippet template(args)}
		<div class="wrap">
			<Select
				searchable
				options={args.optionsWithSeparators}
				value={selectedWithSeparators}
				onselect={(value: string) => {
					selectedWithSeparators = value;
				}}
				placeholder="Choose an action..."
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

<Story name="Complex Separators">
	{#snippet template(_args)}
		<div class="wrap">
			<Select
				searchable
				options={[
					{ value: 'draft', label: 'Draft', icon: 'pencil' },
					{ value: 'published', label: 'Published', icon: 'check' },
					{ value: 'archived', label: 'Archived', icon: 'archive' },
					{ separator: true },
					{ value: 'high', label: 'High Priority', emoji: '🔴' },
					{ value: 'medium', label: 'Medium Priority', emoji: '🟡' },
					{ value: 'low', label: 'Low Priority', emoji: '🟢' },
					{ separator: true },
					{ value: 'today', label: 'Due Today' },
					{ value: 'week', label: 'Due This Week' },
					{ value: 'month', label: 'Due This Month' },
					{ separator: true },
					{ value: 'delete', label: 'Delete', emoji: '🗑️' }
				]}
				value={selectedWithSeparators}
				onselect={(value: string) => {
					selectedWithSeparators = value;
				}}
				placeholder="Filter options..."
			>
				{#snippet itemSnippet({ item, highlighted })}
					{#if item.icon}
						<SelectItem selected={highlighted} {highlighted} icon={item.icon}>
							{item.label}
						</SelectItem>
					{:else if item.emoji}
						<SelectItem selected={highlighted} {highlighted}>
							{#snippet iconSnippet()}
								<span class="emoji">{item.emoji}</span>
							{/snippet}
							{item.label}
						</SelectItem>
					{:else}
						<SelectItem selected={highlighted} {highlighted}>
							{item.label}
						</SelectItem>
					{/if}
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

	.emoji {
		font-size: 16px;
		line-height: 1;
	}

	.custom-component {
		display: flex;
		align-items: center;
		gap: 2px;
	}
</style>
