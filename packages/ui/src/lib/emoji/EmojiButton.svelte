<script lang="ts">
	type Props = {
		emoji: string;
		onclick: () => void;
	};

	let { emoji, onclick }: Props = $props();
</script>

<button type="button" class="emoji-picker-button" {onclick}>
	<span>{emoji}</span>
</button>

<style lang="postcss">
	.emoji-picker-button {
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 23px;
		width: 32px;
		height: 32px;
		border-radius: var(--radius-m);
		transition: background-color var(--transition-fast);

		span {
			transition: transform var(--transition-fast);
		}

		&:hover {
			background-color: var(--clr-bg-1-muted);

			span {
				transform: scale(1.1);
			}
		}
	}
</style>
