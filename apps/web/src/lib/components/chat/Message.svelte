<script lang="ts" module>
	export interface MessageProps {
		author: string;
		content: string;
	}
</script>

<script lang="ts">
	const { author, content }: MessageProps = $props();
</script>

<div class="chat-message">
	<div class="chat-message-author">{author}</div>
	<div class="chat-message-content">{content}</div>
</div>

<style>
	.chat-message {
		display: flex;
		padding: 10px;
		border-bottom: 1px solid #ccc;
		gap: 10px;
	}

	.chat-message-author {
		font-weight: bold;
	}
</style>
