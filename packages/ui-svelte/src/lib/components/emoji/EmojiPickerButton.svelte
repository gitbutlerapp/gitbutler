<script lang="ts">
	import Button from "$components/Button.svelte";
	import ContextMenu from "$components/ContextMenu.svelte";
	import EmojiPicker from "$components/emoji/EmojiPicker.svelte";
	import type { EmojiInfo } from "$components/emoji/utils";

	type Props = {
		onEmojiSelect: (emoji: EmojiInfo) => void;
	};

	let { onEmojiSelect: callback }: Props = $props();

	let leftClickTrigger = $state<HTMLDivElement>();
	let pickerOpen = $state(false);

	function handleClick() {
		pickerOpen = !pickerOpen;
	}

	function onEmojiSelect(emoji: EmojiInfo) {
		callback(emoji);
		pickerOpen = false;
	}
</script>

<div bind:this={leftClickTrigger}>
	<Button kind="ghost" icon="smile" onclick={handleClick} />
</div>

{#if pickerOpen}
	<ContextMenu
		{leftClickTrigger}
		side="top"
		align="start"
		target={leftClickTrigger}
		onclose={() => {
			pickerOpen = false;
		}}
	>
		<EmojiPicker {onEmojiSelect} />
	</ContextMenu>
{/if}
