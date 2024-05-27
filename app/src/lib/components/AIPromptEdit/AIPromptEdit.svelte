<script lang="ts">
	import { PromptService } from '$lib/ai/promptService';
	import Content from '$lib/components/AIPromptEdit/Content.svelte';
	import Button from '$lib/components/Button.svelte';
	import { getContext } from '$lib/utils/context';
	import { get } from 'svelte/store';
	import type { Prompts, UserPrompt } from '$lib/ai/types';

	export let promptUse: 'commits' | 'branches';

	const promptService = getContext(PromptService);

	let prompts: Prompts;

	if (promptUse == 'commits') {
		prompts = promptService.commitPrompts;
	} else {
		prompts = promptService.branchPrompts;
	}

	$: userPrompts = prompts.userPrompts;

	function createNewPrompt() {
		prompts.userPrompts.set([...get(prompts.userPrompts), promptService.createEmptyUserPrompt()]);
	}

	function deletePrompt(targetPrompt: UserPrompt) {
		const filteredPrompts = get(prompts.userPrompts).filter((prompt) => prompt != targetPrompt);
		prompts.userPrompts.set(filteredPrompts);
	}
</script>

{#if prompts && $userPrompts}
	<div class="container">
		<h3 class="text-head-20 text-bold">
			{promptUse == 'commits' ? 'Commit Message Prompts' : 'Branch Name Prompts'}
		</h3>
		<div>
			{#if prompts.defaultPrompt}
				<Content
					displayMode="readOnly"
					prompt={{ prompt: prompts.defaultPrompt, name: 'Default Prompt', id: 'default' }}
					roundedTop={true}
					roundedBottom={$userPrompts.length == 0}
				/>
			{/if}
			{#each $userPrompts as prompt, index}
				<Content
					bind:prompt
					displayMode="writable"
					roundedTop={false}
					roundedBottom={index + 1 == $userPrompts.length}
					on:deletePrompt={(e) => deletePrompt(e.detail.prompt)}
				/>
			{/each}
		</div>

		<Button kind="solid" style="ghost" icon="plus-small" on:click={createNewPrompt}
			>New prompt</Button
		>
	</div>
{/if}

<style lang="postcss">
	.container {
		display: flex;
		flex-direction: column;

		gap: var(--size-8);
	}
</style>
