<script lang="ts">
	import { ButPcAvatar, type faceType } from '@gitbutler/ui';
	import type { Snippet } from 'svelte';

	type Props = {
		children?: Snippet;
		style: 'neutral' | 'pop' | 'error';
		face: faceType;
		extraContent?: Snippet;
		reverseElementsOrder?: boolean;
	};

	const { children, style, face, extraContent, reverseElementsOrder }: Props = $props();
</script>

<div class="service-message__wrapper">
	<div class="service-message">
		<ButPcAvatar mode={face} />

		<div class="service-message__content" class:reverse={reverseElementsOrder}>
			{#if children}
				<div
					class="service-message__bubble service-message__bubble--{style} service-message__bubble--animate"
					class:service-message__bubble--wiggle={face === 'waiting'}
				>
					{@render children()}
				</div>
			{/if}

			{#if extraContent}
				{@render extraContent()}
			{/if}
		</div>
	</div>
</div>

<style lang="postcss">
	.service-message__wrapper {
		display: flex;
		width: 100%;
		padding: 8px 0 16px;
	}
	.service-message {
		display: flex;
		align-items: flex-end;
		width: 100%;
		gap: 14px;
	}
	.service-message__content {
		display: flex;
		flex-direction: column;
		width: 100%;
		overflow: hidden;
		gap: 10px;
		text-wrap: wrap;
		word-break: break-word;

		&.reverse {
			flex-direction: column-reverse;
		}
	}
	.service-message__bubble {
		display: flex;
		width: fit-content;
		max-width: var(--message-max-width);
		padding: 10px 12px;
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-2);
		color: var(--clr-text-2);
	}

	.service-message__bubble--error {
		background-color: var(--clr-theme-err-soft);
		color: var(--clr-theme-err-on-soft);
	}

	.service-message__bubble--pop {
		background-color: var(--clr-theme-pop-soft);
		color: var(--clr-text-1);
	}

	.service-message__bubble--animate {
		animation: popIn 0.2s cubic-bezier(0.215, 0.61, 0.355, 1) 0.1s both;
	}

	.service-message__bubble--wiggle {
		animation:
			popIn 0.2s cubic-bezier(0.215, 0.61, 0.355, 1) 0.1s both,
			wiggle 5s ease-in-out infinite;
	}

	@keyframes popIn {
		0% {
			transform: scale(0.2) translateY(15px) rotate(-8deg);
			transform-origin: left bottom;
			opacity: 0;
		}
		100% {
			transform: scale(1) translateY(0px) rotate(0deg);
			transform-origin: left bottom;
			opacity: 1;
		}
	}

	@keyframes wiggle {
		0%,
		12%,
		100% {
			transform: translateX(0px) rotate(0deg);
		}
		2% {
			transform: translateX(-3px) rotate(-0.2deg);
		}
		4% {
			transform: translateX(3px) rotate(0.2deg);
		}
		6% {
			transform: translateX(-3px) rotate(-0.2deg);
		}
		8% {
			transform: translateX(3px) rotate(0.2deg);
		}
		10% {
			transform: translateX(0px) rotate(0deg);
		}
	}
</style>
