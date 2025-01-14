<script lang="ts">
	interface Props {
		message: string | undefined;
		sendMessage: () => void;
	}

	let { message = $bindable(), sendMessage }: Props = $props();

	function handleKeyDown(event: KeyboardEvent) {
		if (event.key === 'Enter') {
			sendMessage();
		}
	}
</script>

<div class="chat-input">
	<input type="text" bind:value={message} onkeydown={handleKeyDown} />
	<button type="button" onclick={sendMessage}>Send</button>
</div>

<style>
	.chat-input {
		display: flex;
		justify-content: space-between;
		padding: 10px;
		border-top: 1px solid #ccc;
	}
</style>
