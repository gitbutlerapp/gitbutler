<script lang="ts">
	import { PromptService } from '$lib/ai/promptService';
	import Content from '$lib/components/AIPromptEdit/Content.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import { get } from 'svelte/store';
	import type { Prompts, UserPrompt } from '$lib/ai/types';

	interface Props {
		promptUse: 'commits' | 'branches';
	}

	let { promptUse }: Props = $props();

	const promptService = getContext(PromptService);

	let prompts = $state<Prompts>();

	if (promptUse === 'commits') {
		prompts = promptService.commitPrompts;
	} else {
		prompts = promptService.branchPrompts;
	}

	let userPrompts = $derived(prompts.userPrompts);

	function createNewPrompt() {
		prompts?.userPrompts.set([
			...get(prompts.userPrompts),
			promptService.createDefaultUserPrompt(promptUse)
		]);
	}

	function deletePrompt(targetPrompt: UserPrompt) {
		if (prompts) {
			const filteredPrompts = get(prompts.userPrompts).filter((prompt) => prompt !== targetPrompt);
			prompts.userPrompts.set(filteredPrompts);
		}
	}
</script>

{#if prompts && $userPrompts}
	<div class="prompt-item__title">
		<h3 class="text-15 text-bold">
			{promptUse === 'commits' ? 'Commit message' : 'Branch name'}
		</h3>
		<Button style="ghost" outline icon="plus-small" onclick={createNewPrompt}>New prompt</Button>
	</div>

	<div class="content">
		<Content
			displayMode="readOnly"
			prompt={{
				prompt: prompts.defaultPrompt,
				name: 'Default Prompt',
				id: 'default'
			}}
		/>

		{#each $userPrompts as _prompt, idx}
			<Content
				bind:prompt={$userPrompts[idx] as UserPrompt}
				displayMode="writable"
				on:deletePrompt={(e) => deletePrompt(e.detail.prompt)}
			/>
		{/each}
	</div>
{/if}

<style lang="postcss">
	.prompt-item__title {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 24px;
	}

	.content {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
</style>
