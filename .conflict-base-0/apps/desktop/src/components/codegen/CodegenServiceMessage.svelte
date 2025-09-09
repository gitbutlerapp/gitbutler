<script lang="ts">
	import { ButPcAvatar } from '@gitbutler/ui';
	import { popIn } from '@gitbutler/ui/utils/transitions';
	import type { Snippet } from 'svelte';

	type Props = {
		children: Snippet;
		style: 'neutral' | 'pop' | 'error';
		face: 'idle' | 'thinking';
	};

	const { children, style, face }: Props = $props();
</script>

<div class="service-message__wrapper">
	<div class="service-message">
		<ButPcAvatar mode={face} />
		<div class="service-message__bubble service-message__bubble--{style}" in:popIn>
			{@render children()}
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
		gap: 16px;
	}
	.service-message__bubble {
		display: flex;
		max-width: var(--message-max-width);
		padding: 8px 12px;
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
</style>
