<script module lang="ts">
	import Icon from '$components/Icon.svelte';
	import Textbox from '$components/Textbox.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / Textbox',
		component: Textbox,
		args: {
			value: '',
			placeholder: 'Enter text...',
			type: 'text',
			size: 'default',
			textAlign: 'left',
			disabled: false,
			readonly: false,
			required: false,
			wide: false,
			showCountActions: false,
			spellcheck: true,
			autocorrect: true,
			autocomplete: true,
			autofocus: false,
			selectall: false,
			label: '',
			helperText: '',
			iconLeft: undefined,
			iconRight: undefined,
			width: undefined,
			minVal: undefined,
			maxVal: undefined
		},
		argTypes: {
			type: {
				control: { type: 'select' },
				options: ['text', 'password', 'number', 'email', 'tel', 'url', 'search', 'date', 'time']
			},
			size: {
				control: { type: 'select' },
				options: ['default', 'large']
			},
			textAlign: {
				control: { type: 'select' },
				options: ['left', 'center', 'right']
			},
			iconLeft: {
				control: { type: 'text' }
			},
			iconRight: {
				control: { type: 'text' }
			}
		}
	});
</script>

<script lang="ts">
	let textValue = $state('');
	let passwordValue = $state('');
	let numberValue = $state('42');
	let emailValue = $state('user@example.com');
	let searchValue = $state('');
</script>

<Story name="Default">
	{#snippet template(args)}
		<div class="wrap">
			<Textbox
				bind:value={textValue}
				placeholder={args.placeholder}
				type={args.type}
				size={args.size}
				textAlign={args.textAlign}
				disabled={args.disabled}
				readonly={args.readonly}
				required={args.required}
				wide={args.wide}
				showCountActions={args.showCountActions}
				spellcheck={args.spellcheck}
				autocorrect={args.autocorrect}
				autocomplete={args.autocomplete}
				autofocus={args.autofocus}
				selectall={args.selectall}
				label={args.label}
				helperText={args.helperText}
				iconLeft={args.iconLeft}
				iconRight={args.iconRight}
				width={args.width}
				minVal={args.minVal}
				maxVal={args.maxVal}
			/>
		</div>
	{/snippet}
</Story>

<Story name="With Label and Helper Text">
	{#snippet template()}
		<div class="wrap">
			<Textbox
				bind:value={textValue}
				label="Full Name"
				placeholder="Enter your full name"
				helperText="This will be displayed on your profile"
			/>
		</div>
	{/snippet}
</Story>

<Story name="With Icons">
	{#snippet template()}
		<div class="wrap">
			<div class="story-group">
				<h4>Left Icon</h4>
				<Textbox
					bind:value={emailValue}
					type="email"
					iconLeft="mail"
					placeholder="Enter email address"
					label="Email"
				/>
			</div>
			<div class="story-group">
				<h4>Right Icon</h4>
				<Textbox
					bind:value={searchValue}
					type="search"
					iconRight="search"
					placeholder="Search..."
					label="Search"
				/>
			</div>
			<div class="story-group">
				<h4>Both Icons</h4>
				<Textbox
					bind:value={textValue}
					type="text"
					iconLeft="profile"
					iconRight="success"
					placeholder="Username with validation"
					label="Username"
				/>
			</div>
		</div>
	{/snippet}
</Story>

<Story name="Custom Icons with Emojis">
	{#snippet template()}
		<div class="wrap">
			<div class="story-group">
				<h4>Emoji Icons</h4>
				<Textbox
					bind:value={textValue}
					type="text"
					placeholder="Search with emojis..."
					label="Search"
				>
					{#snippet customIconLeft()}
						<span style="font-size: 16px;">🔍</span>
					{/snippet}
					{#snippet customIconRight()}
						<span style="font-size: 16px;">✨</span>
					{/snippet}
				</Textbox>
			</div>
			<div class="story-group">
				<h4>Mixed Icons</h4>
				<Textbox
					bind:value={textValue}
					type="text"
					placeholder="Custom left, regular right"
					label="Mixed Example"
					iconRight="success"
				>
					{#snippet customIconLeft()}
						<span style="font-size: 16px;">🎯</span>
					{/snippet}
				</Textbox>
			</div>
			<div class="story-group">
				<h4>Custom Button</h4>
				<Textbox
					bind:value={textValue}
					type="text"
					placeholder="Text with action button"
					label="Action Example"
				>
					{#snippet customIconRight()}
						<Icon name="plus" />
					{/snippet}
				</Textbox>
			</div>
		</div>
	{/snippet}
</Story>

<Story name="Password Field">
	{#snippet template()}
		<div class="wrap">
			<div class="story-group">
				<h4>Normal Password</h4>
				<Textbox
					bind:value={passwordValue}
					type="password"
					iconLeft="locked"
					placeholder="Enter password"
					label="Password"
					helperText="Password must be at least 8 characters"
				/>
			</div>
			<div class="story-group">
				<h4>Disabled Password</h4>
				<Textbox
					value="disabled-password"
					type="password"
					iconLeft="locked"
					placeholder="Enter password"
					label="Disabled Password"
					disabled={true}
					helperText="Show/hide button is disabled"
				/>
			</div>
			<div class="story-group">
				<h4>Readonly Password</h4>
				<Textbox
					value="readonly-password"
					type="password"
					iconLeft="locked"
					placeholder="Enter password"
					label="Readonly Password"
					readonly={true}
					helperText="Show/hide button is disabled"
				/>
			</div>
		</div>
	{/snippet}
</Story>

<Story name="Number with Count Actions">
	{#snippet template()}
		<div class="wrap">
			<div class="story-group">
				<h4>Normal Number</h4>
				<Textbox
					bind:value={numberValue}
					type="number"
					showCountActions={true}
					minVal={0}
					maxVal={100}
					placeholder="Enter number"
					label="Count"
					helperText="Use buttons or type to adjust value"
				/>
			</div>
			<div class="story-group">
				<h4>Disabled Number</h4>
				<Textbox
					value="25"
					type="number"
					showCountActions={true}
					minVal={0}
					maxVal={100}
					placeholder="Enter number"
					label="Disabled Count"
					disabled={true}
					helperText="Count buttons are disabled"
				/>
			</div>
			<div class="story-group">
				<h4>Readonly Number</h4>
				<Textbox
					value="50"
					type="number"
					showCountActions={true}
					minVal={0}
					maxVal={100}
					placeholder="Enter number"
					label="Readonly Count"
					readonly={true}
					helperText="Count buttons are disabled"
				/>
			</div>
		</div>
	{/snippet}
</Story>

<Story name="Sizes">
	{#snippet template()}
		<div class="wrap">
			<div class="story-group">
				<h4>Default Size</h4>
				<Textbox
					bind:value={textValue}
					size="default"
					placeholder="Default size textbox"
					label="Default"
				/>
			</div>
			<div class="story-group">
				<h4>Large Size</h4>
				<Textbox
					bind:value={textValue}
					size="large"
					placeholder="Large size textbox"
					label="Large"
				/>
			</div>
		</div>
	{/snippet}
</Story>

<Story name="Text Alignment">
	{#snippet template()}
		<div class="wrap">
			<div class="story-group">
				<h4>Left Aligned</h4>
				<Textbox value="Left aligned text" textAlign="left" label="Left" />
			</div>
			<div class="story-group">
				<h4>Center Aligned</h4>
				<Textbox value="Center aligned text" textAlign="center" label="Center" />
			</div>
			<div class="story-group">
				<h4>Right Aligned</h4>
				<Textbox value="Right aligned text" textAlign="right" label="Right" />
			</div>
		</div>
	{/snippet}
</Story>

<Story name="States">
	{#snippet template()}
		<div class="wrap">
			<div class="story-group">
				<h4>Normal</h4>
				<Textbox value="Normal state" placeholder="Enter text" label="Normal" />
			</div>
			<div class="story-group">
				<h4>Disabled</h4>
				<Textbox value="Disabled state" disabled={true} iconLeft="locked" label="Disabled" />
			</div>
			<div class="story-group">
				<h4>Readonly</h4>
				<Textbox
					value="Readonly state"
					readonly={true}
					label="Readonly"
					helperText="This field cannot be edited"
				/>
			</div>
			<div class="story-group">
				<h4>Required</h4>
				<Textbox
					value=""
					required={true}
					placeholder="This field is required"
					label="Required Field *"
				/>
			</div>
		</div>
	{/snippet}
</Story>

<Story name="Input Types">
	{#snippet template()}
		<div class="wrap">
			<div class="story-group">
				<h4>Email</h4>
				<Textbox
					bind:value={emailValue}
					type="email"
					iconLeft="mail"
					placeholder="user@example.com"
					label="Email"
				/>
			</div>
			<div class="story-group">
				<h4>Telephone</h4>
				<Textbox
					value="+1 (555) 123-4567"
					type="tel"
					iconLeft="home"
					placeholder="Phone number"
					label="Phone"
				/>
			</div>
			<div class="story-group">
				<h4>URL</h4>
				<Textbox
					value="https://example.com"
					type="url"
					iconLeft="open-link"
					placeholder="https://example.com"
					label="Website URL"
				/>
			</div>
			<div class="story-group">
				<h4>Search</h4>
				<Textbox
					bind:value={searchValue}
					type="search"
					iconLeft="search"
					placeholder="Search..."
					label="Search"
				/>
			</div>
			<div class="story-group">
				<h4>Date</h4>
				<Textbox value="2025-08-01" type="date" label="Date" />
			</div>
			<div class="story-group">
				<h4>Time</h4>
				<Textbox value="14:30" type="time" label="Time" />
			</div>
		</div>
	{/snippet}
</Story>

<Story name="Wide Layout">
	{#snippet template()}
		<div class="wrap">
			<div class="story-group">
				<h4>Normal Width</h4>
				<Textbox value="Normal width textbox" placeholder="Enter text" label="Normal" />
			</div>
			<div class="story-group">
				<h4>Wide</h4>
				<Textbox
					value="Wide textbox that takes full width"
					wide={true}
					placeholder="Enter text"
					label="Wide"
				/>
			</div>
			<div class="story-group">
				<h4>Custom Width</h4>
				<Textbox
					value="Custom width textbox"
					width={300}
					placeholder="Enter text"
					label="Custom Width (300px)"
				/>
			</div>
		</div>
	{/snippet}
</Story>

<style>
	.wrap {
		display: flex;
		flex-direction: column;
		max-width: 600px;
		padding: 16px;
		gap: 24px;
	}

	.story-group {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.story-group h4 {
		margin: 0;
		color: var(--clr-text-1);
		font-weight: 600;
		font-size: 14px;
	}
</style>
