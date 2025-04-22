<!-- <script module lang="ts">
	import EmojiPickerButton from '@gitbutler/ui/EmojiPickerButton.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Editing / EmojiPicker',
		component: EmojiPickerButton,
		args: {}
	});
</script>

<Story name="Playground" /> -->

<script module lang="ts">
	import EmojiPicker from '$lib/emoji/EmojiPicker.svelte';
	import { defineMeta, setTemplate } from '@storybook/addon-svelte-csf';
	import type { EmojiInfo } from '$lib/emoji/utils';

	const { Story } = defineMeta({
		title: 'Editing / EmojiPicker',
		component: EmojiPicker,
		args: {},
		argTypes: {}
	});
</script>

<script lang="ts">
	setTemplate(template);
</script>

{#snippet template()}
	<div class="wrap">
		<EmojiPicker
			onEmojiSelect={(emoji: EmojiInfo) => {
				// eslint-disable-next-line no-console
				console.log('Emoji selected:', emoji);
			}}
		/>
	</div>
{/snippet}

<Story name="Playground" />

<style>
	.wrap {
		display: flex;
		width: fit-content;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		overflow: hidden;
	}
</style>
