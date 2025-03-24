<script lang="ts">
	import Textarea from '@gitbutler/ui/Textarea.svelte';

	interface Props {
		value: string | undefined;
		oninput?: (e: Event) => void;
	}

	const { value = $bindable(), oninput }: Props = $props();
</script>

<div class="editor-title-input">
	<Textarea
		{value}
		autofocus
		placeholder="Your commit title"
		padding={{
			top: 8,
			right: 12,
			bottom: 8,
			left: 12
		}}
		fontSize={14}
		fontWeight="semibold"
		flex="1"
		minRows={1}
		maxRows={10}
		{oninput}
	/>
</div>

<style lang="postcss">
	.editor-title-input {
		display: flex;
	}
</style>
