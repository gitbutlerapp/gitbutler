<script lang="ts">
	import { PromptService } from '$lib/ai/promptService';
	import Content from '$lib/components/AIPromptEdit/Content.svelte';
	import Expandable from '$lib/components/AIPromptEdit/Expandable.svelte';
	import Button from '$lib/components/Button.svelte';
	import TextBox from '$lib/components/TextBox.svelte';
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
				<!-- svelte-ignore a11y-no-static-element-interactions -->
				<Expandable roundedTop={true} roundedBottom={$userPrompts.length == 0}>
					<svelte:fragment slot="header">
						<div class="prompt-name">
							<p class="text-base-15 text-semibold">Default Prompt</p>
						</div>
					</svelte:fragment>

					<!-- svelte-ignore a11y-click-events-have-key-events -->
					<div on:click|stopPropagation class="not-clickable">
						<Content displayMode="readOnly" bind:promptMessages={prompts.defaultPrompt} />
					</div>
				</Expandable>
			{/if}
			{#each $userPrompts as prompt, index}
				<Expandable roundedTop={false} roundedBottom={index + 1 == $userPrompts.length}>
					<svelte:fragment slot="header">
						<div class="prompt-name">
							<TextBox bind:value={prompt.name} wide on:click={(e) => e.stopPropagation()} />
							<Button
								on:click={(e) => {
									e.stopPropagation();
									deletePrompt(prompt);
								}}
								icon="bin"
							/>
						</div>
					</svelte:fragment>

					<!-- svelte-ignore a11y-click-events-have-key-events -->
					<!-- svelte-ignore a11y-no-static-element-interactions -->
					<div on:click|stopPropagation class="not-clickable">
						<Content displayMode="writable" bind:promptMessages={prompt.prompt} />
					</div>
				</Expandable>
			{/each}
		</div>

		<div>
			<Button style="pop" on:click={createNewPrompt}>Create new prompt</Button>
		</div>
	</div>
{/if}

<style lang="postcss">
	.container {
		display: flex;
		flex-direction: column;

		gap: var(--size-8);
	}
	.prompt-name {
		display: flex;
		align-items: center;
		gap: var(--size-8);
	}

	.not-clickable {
		cursor: default;
	}
</style>
