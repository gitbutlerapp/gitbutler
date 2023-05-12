<script lang="ts">
	import Modal from './Modal.svelte';
	import Button from '../Button/Button.svelte';
	import { IconBookmark } from '../../icons';

	export let Hst: import('@histoire/plugin-svelte').Hst;

	const widths = [
		{ label: 'Default', value: 'default' as const },
		{ label: 'Small', value: 'small' as const },
		{ label: 'Large', value: 'large' as const }
	];

	let width = widths[0].value;
	let title = 'Title';
	let withTitle = true;
	let withIcon = false;

	let modal: Modal;
</script>

<Hst.Story title="Modal">
	<svelte:fragment slot="controls">
		<Hst.Text title="Title" bind:value={title} />
		<Hst.Checkbox title="With title" bind:value={withTitle} />
		<Hst.Checkbox title="With icon" bind:value={withIcon} />
		<Hst.Radio title="Width" bind:value={width} options={widths} />
	</svelte:fragment>

	<Button on:click={modal.show}>Open Modal</Button>

	<Hst.Variant title="Text content">
		<Modal
			icon={withIcon ? IconBookmark : undefined}
			title={withTitle ? title : undefined}
			{width}
			bind:this={modal}
		>
			Lorem ipsum dolor sit amet, consectetur adipiscing elit. Phasellus at odio nec lacus viverra
			posuere nec et lectus. Etiam ac egestas nunc, id molestie nulla. Nunc a tincidunt orci, sed
			tempus lacus. Nullam ac viverra magna, sed ultricies sapien. Suspendisse rutrum convallis
			orci, non auctor est congue a. Sed et tincidunt lacus. In sed ligula ut elit malesuada varius
			non eget tortor
		</Modal>
	</Hst.Variant>

	<Hst.Variant title="A lot of text">
		<Modal
			icon={withIcon ? IconBookmark : undefined}
			title={withTitle ? title : undefined}
			{width}
			bind:this={modal}
		>
			Lorem ipsum dolor sit amet, consectetur adipiscing elit. Phasellus at odio nec lacus viverra
			posuere nec et lectus. Etiam ac egestas nunc, id molestie nulla. Nunc a tincidunt orci, sed
			tempus lacus. Nullam ac viverra magna, sed ultricies sapien. Suspendisse rutrum convallis
			orci, non auctor est congue a. Sed et tincidunt lacus. In sed ligula ut elit malesuada varius
			non eget tortor<br />

			Lorem ipsum dolor sit amet, consectetur adipiscing elit. Phasellus at odio nec lacus viverra
			posuere nec et lectus. Etiam ac egestas nunc, id molestie nulla. Nunc a tincidunt orci, sed
			tempus lacus. Nullam ac viverra magna, sed ultricies sapien. Suspendisse rutrum convallis
			orci, non auctor est congue a. Sed et tincidunt lacus. In sed ligula ut elit malesuada varius
			non eget tortor<br />

			Lorem ipsum dolor sit amet, consectetur adipiscing elit. Phasellus at odio nec lacus viverra
			posuere nec et lectus. Etiam ac egestas nunc, id molestie nulla. Nunc a tincidunt orci, sed
			tempus lacus. Nullam ac viverra magna, sed ultricies sapien. Suspendisse rutrum convallis
			orci, non auctor est congue a. Sed et tincidunt lacus. In sed ligula ut elit malesuada varius
			non eget tortor<br />

			Lorem ipsum dolor sit amet, consectetur adipiscing elit. Phasellus at odio nec lacus viverra
			posuere nec et lectus. Etiam ac egestas nunc, id molestie nulla. Nunc a tincidunt orci, sed
			tempus lacus. Nullam ac viverra magna, sed ultricies sapien. Suspendisse rutrum convallis
			orci, non auctor est congue a. Sed et tincidunt lacus. In sed ligula ut elit malesuada varius
			non eget tortor<br />

			Lorem ipsum dolor sit amet, consectetur adipiscing elit. Phasellus at odio nec lacus viverra
			posuere nec et lectus. Etiam ac egestas nunc, id molestie nulla. Nunc a tincidunt orci, sed
			tempus lacus. Nullam ac viverra magna, sed ultricies sapien. Suspendisse rutrum convallis
			orci, non auctor est congue a. Sed et tincidunt lacus. In sed ligula ut elit malesuada varius
			non eget tortor<br />
		</Modal>
	</Hst.Variant>
</Hst.Story>
