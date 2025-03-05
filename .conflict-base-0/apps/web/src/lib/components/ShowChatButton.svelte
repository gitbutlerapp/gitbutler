<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';

	interface Props {
		onclick: () => void;
	}

	const { onclick }: Props = $props();
</script>

<button class="show-chat" type="button" {onclick}>
	<div class="chat-image-wrap">
		<div class="chat-image">
			<svg
				width="76"
				height="32"
				viewBox="0 0 76 32"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
				class="chat-image__bubble-right"
			>
				<path
					d="M0.5 8.5C0.5 4.08172 4.08172 0.5 8.5 0.5H67.5C71.9183 0.5 75.5 4.08172 75.5 8.5V31.5L58 21.5H8.5C4.08172 21.5 0.5 17.9183 0.5 13.5V8.5Z"
					fill="#8FD6D2"
				/>
				<path
					opacity="0.8"
					d="M11.5767 8.00781H64.8721M11.5767 14.0078H44.8721"
					stroke="white"
					stroke-width="2"
				/>
			</svg>

			<svg
				width="75"
				height="33"
				viewBox="0 0 75 33"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
				class="chat-image__bubble-left"
			>
				<path
					d="M66.5 1H8.5C4.35786 1 1 4.35786 1 8.5V32H74V8.5C74 4.35786 70.6421 1 66.5 1Z"
					fill="white"
					fill-opacity="0.6"
					stroke="#B4AFAC"
				/>
				<path
					d="M8.57666 8.00781H64.8721M8.57666 14.0078H44.8721M8.57666 20.4773H53.0785"
					stroke="#B4AFAC"
					stroke-width="2"
				/>
			</svg>
		</div>
	</div>
	<div class="show-chat__content">
		<span class="text-12 text-semibold">Show discussion</span>
		<div class="show-chat__icon"><Icon name="chat" /></div>
	</div>
</button>

<style lang="postcss">
	.show-chat {
		pointer-events: all;
		position: fixed;
		bottom: 20px;
		right: 20px;
		display: flex;
		flex-direction: column;
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		overflow: hidden;
		box-shadow: var(--fx-shadow-s);
		transition:
			background-color var(--transition-fast),
			transform var(--transition-medium);

		&:hover {
			transform: translateY(-2px);

			.show-chat__content {
				background-color: var(--clr-bg-1-muted);
			}

			.chat-image-wrap {
				height: 54px;
			}

			.chat-image {
				background-color: var(--clr-bg-1);
			}
		}
	}

	.show-chat__content {
		display: flex;
		align-items: center;
		padding: 16px;
		gap: 10px;
		border-top: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
	}

	.show-chat__icon {
		display: flex;
		color: var(--clr-text-1);
		opacity: 0.4;
	}

	.chat-image-wrap {
		position: relative;
		display: flex;
		width: 100%;
		height: 48px;
		overflow: hidden;
		transition: height 0.2s;
	}

	.chat-image {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 77px;
		background-color: var(--clr-bg-1-muted);
		transition: background-color 0.2s;
	}

	.chat-image__bubble-right {
		position: absolute;
		top: 10px;
		right: 10px;
	}

	.chat-image__bubble-left {
		position: absolute;
		top: 35px;
		left: 10px;
	}
</style>
