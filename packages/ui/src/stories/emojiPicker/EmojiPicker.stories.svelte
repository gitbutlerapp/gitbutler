<script module lang="ts">
	import EmojiPicker from '$components/emoji/EmojiPicker.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import type { EmojiInfo } from '$components/emoji/utils';

	const { Story } = defineMeta({
		title: 'Editing / EmojiPicker',
		component: EmojiPicker,
		args: {},
		argTypes: {}
	});
</script>

<script lang="ts">
</script>

<Story name="default">
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
</Story>

<Story name="Playground" />

<style>
	.wrap {
		display: flex;
		width: fit-content;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}
</style>
