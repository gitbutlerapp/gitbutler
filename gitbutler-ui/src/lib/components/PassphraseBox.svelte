<script lang="ts">
	import Button from './Button.svelte';
	import TextBox from './TextBox.svelte';
	import { PromptService, type SystemPrompt } from '$lib/backend/prompt';
	import { getContextByClass } from '$lib/utils/context';
	import { createEventDispatcher } from 'svelte';

	export let value: string = '';
	export let submitDisabled: boolean = false;
	export let isSubmitting: boolean = true;
	export let prompt: SystemPrompt | undefined;

	const promptService = getContextByClass(PromptService);

	const dispatch = createEventDispatcher<{
		change: string;
		input: string;
		submit: string;
		cancel: void;
	}>();
</script>

<div class="passbox">
	<span class="text-base-body-11 passbox__helper-text">
		{prompt}
	</span>
	<TextBox
		focus
		type="password"
		bind:value
		on:change={(e) => dispatch('change', e.detail)}
		on:input={(e) => dispatch('input', e.detail)}
		on:keydown={(e) => {
			if (e.detail.key === 'Enter') dispatch('submit', value);
		}}
	/>
	<div class="passbox__actions">
		<Button
			color="neutral"
			disabled={isSubmitting}
			kind="outlined"
			on:click={async () => {
				if (!prompt) return;
				await promptService.cancel(prompt.id);
				prompt = undefined;
				dispatch('cancel');
			}}
		>
			Cancel
		</Button>
		<Button
			grow
			on:click={() => {
				dispatch('submit', value);
			}}
			disabled={submitDisabled || isSubmitting}
			loading={isSubmitting}
		>
			Submit
		</Button>
	</div>
</div>

<style>
	.passbox {
		display: flex;
		flex-direction: column;
		gap: var(--size-8);
		padding: var(--size-14);
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-container-pale);
	}

	.passbox__helper-text {
		color: var(--clr-theme-scale-ntrl-50);
	}

	.passbox__actions {
		display: flex;
		gap: var(--size-6);
	}
</style>
