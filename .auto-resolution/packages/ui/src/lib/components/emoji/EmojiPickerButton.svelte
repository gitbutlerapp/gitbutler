<script lang="ts">
	import Button from '$components/Button.svelte';
	import ContextMenu from '$components/ContextMenu.svelte';
	import EmojiPicker from '$components/emoji/EmojiPicker.svelte';
	import type { EmojiInfo } from '$components/emoji/utils';

	type Props = {
		onEmojiSelect: (emoji: EmojiInfo) => void;
	};

	let { onEmojiSelect: callback }: Props = $props();

	let leftClickTrigger = $state<HTMLDivElement>();
	let picker = $state<ReturnType<typeof ContextMenu>>();

	function handleClick() {
		picker?.toggle();
	}

	function onEmojiSelect(emoji: EmojiInfo) {
		callback(emoji);
		picker?.close();
	}
</script>

<div bind:this={leftClickTrigger}>
	<Button kind="ghost" icon="smile" onclick={handleClick} />
</div>

<ContextMenu bind:this={picker} {leftClickTrigger} side="top" horizontalAlign="left">
	<EmojiPicker {onEmojiSelect} />
</ContextMenu>
