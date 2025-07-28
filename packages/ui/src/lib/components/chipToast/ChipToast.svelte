<script lang="ts">
	import { fly, fade } from 'svelte/transition';
	import type { ChipToastType } from '$components/chipToast/chipToastTypes';

	export let type: ChipToastType = 'neutral';
	export let message: string;
</script>

<div
	class="chip-toast chip-toast--{type}"
	role="alert"
	aria-live="polite"
	in:fly={{ y: 20, duration: 300 }}
	out:fade={{ duration: 200 }}
>
	{message}
</div>

<style>
	.chip-toast {
		min-width: 300px;
		max-width: 400px;
		padding: 12px 16px;
		border-radius: 8px;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
		font-size: 13px;
		line-height: 1.4;
		word-wrap: break-word;
		text-align: center;
	}

	/* ChipToast type styles */
	.chip-toast--neutral {
		background: var(--clr-theme-ntrl-element);
		color: var(--clr-theme-ntrl-on-element);
	}

	.chip-toast--success {
		background: var(--clr-theme-succ-element);
		color: var(--clr-theme-succ-on-element);
	}

	.chip-toast--warning {
		background: var(--clr-theme-warn-element);
		color: var(--clr-theme-warn-on-element);
	}

	.chip-toast--error {
		background: var(--clr-theme-err-element);
		color: var(--clr-theme-err-on-element);
	}
</style>
