<script lang="ts">
	import { PromptService } from '$lib/ai/promptService';
	import Content from '$lib/components/AIPromptEdit/Content.svelte';
	import Button from '$lib/shared/Button.svelte';
	import { getContext } from '$lib/utils/context';
	import { get } from 'svelte/store';
	import type { Prompts, UserPrompt } from '$lib/ai/types';

	export let promptUse: 'commits' | 'branches';

	const promptService = getContext(PromptService);

	let prompts: Prompts;

	if (promptUse === 'commits') {
		prompts = promptService.commitPrompts;
	} else {
		prompts = promptService.branchPrompts;
	}

	$: userPrompts = prompts.userPrompts;

	function createNewPrompt() {
		prompts.userPrompts.set([
			...get(prompts.userPrompts),
			promptService.createDefaultUserPrompt(promptUse)
		]);
	}

	function deletePrompt(targetPrompt: UserPrompt) {
		const filteredPrompts = get(prompts.userPrompts).filter((prompt) => prompt !== targetPrompt);
		prompts.userPrompts.set(filteredPrompts);
	}
</script>

{#if prompts && $userPrompts}
	<div class="prompt-item__title">
		<h3 class="text-base-15 text-bold">
			{promptUse === 'commits' ? 'Commit message' : 'Branch name'}
		</h3>
		<Button style="ghost" outline icon="plus-small" on:click={createNewPrompt}>New prompt</Button>
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

		{#each $userPrompts as prompt}
			<Content
				bind:prompt
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
