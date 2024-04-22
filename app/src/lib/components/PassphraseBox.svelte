<script lang="ts">
	import Button from './Button.svelte';
	import TextBox from './TextBox.svelte';
	import { showError } from '$lib/notifications/toasts';
	import type { SystemPromptHandle } from '$lib/backend/prompt';

	export let prompt: SystemPromptHandle | undefined;
	export let error: any;
	export let value: string = '';

	let submitDisabled: boolean = false;
	let isSubmitting = false;

	async function submit() {
		if (!prompt) return;
		isSubmitting = true;
		prompt.respond(value);
		isSubmitting = false;
	}

	if (error) showError('Something went wrong', error);
</script>

{#if prompt}
	<div class="passbox">
		<span class="text-base-body-11 passbox__helper-text">
			{prompt?.prompt}
		</span>
		<TextBox
			focus
			type="password"
			bind:value
			on:keydown={(e) => {
				if (e.detail.key === 'Enter') submit();
			}}
		/>
		<div class="passbox__actions">
			<Button
				style="ghost"
				kind="solid"
				disabled={isSubmitting}
				on:click={async () => {
					if (!prompt) return;
					prompt.respond(null);
				}}
			>
				Cancel
			</Button>
			<Button
				style="pop"
				kind="solid"
				grow
				on:click={async () => await submit()}
				disabled={submitDisabled || isSubmitting}
				loading={isSubmitting}
			>
				Submit
			</Button>
		</div>
	</div>
{/if}

<style>
	.passbox {
		display: flex;
		flex-direction: column;
		gap: var(--size-8);
		padding: var(--size-14);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
	}

	.passbox__helper-text {
		color: var(--clr-scale-ntrl-50);
	}

	.passbox__actions {
		display: flex;
		gap: var(--size-6);
	}
</style>
