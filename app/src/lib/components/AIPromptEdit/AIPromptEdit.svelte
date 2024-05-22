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
		prompts.userPrompts.set([
			...get(prompts.userPrompts),
			{
				name: 'My Prompt',
				prompt: []
			}
		]);

		console.log($userPrompts);
	}

	function deletePrompt(targetPrompt: UserPrompt) {
		const filteredPrompts = get(prompts.userPrompts).filter((prompt) => prompt != targetPrompt);
		prompts.userPrompts.set(filteredPrompts);
	}

	function preventBubbling(e: Event) {
		e.stopPropagation();
	}
</script>

{#if prompts && $userPrompts}
	<div class="container">
		<h3 class="text-head-20 text-bold">
			{promptUse == 'commits' ? 'Commit Prompts' : 'Branch Prompts'}
		</h3>
		<div>
			{#if prompts.defaultPrompt}
				<!-- svelte-ignore a11y-no-static-element-interactions -->
				<Expandable position="top">
					<svelte:fragment slot="header">
						<div class="prompt-name">
							<p class="text-base-15 text-bold default">Default Prompt</p>
						</div>
					</svelte:fragment>

					<!-- svelte-ignore a11y-click-events-have-key-events -->
					<div on:click={preventBubbling} class="not-clickable">
						<Content displayMode="readOnly" bind:promptMessages={prompts.defaultPrompt} />
					</div>
				</Expandable>
			{/if}
			{#each $userPrompts as prompt, index}
				<Expandable position={$userPrompts.length == index + 1 ? 'bottom' : 'middle'}>
					<svelte:fragment slot="header">
						<div class="prompt-name">
							<TextBox bind:value={prompt.name} wide on:click={preventBubbling} />
							<Button
								on:click={(e) => {
									preventBubbling(e);
									deletePrompt(prompt);
								}}
								on:input={preventBubbling}
								icon="bin"
							/>
						</div>
					</svelte:fragment>

					<!-- svelte-ignore a11y-click-events-have-key-events -->
					<!-- svelte-ignore a11y-no-static-element-interactions -->
					<div on:click={preventBubbling} class="not-clickable">
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

		& .default {
			opacity: 50%;
		}
	}

	.not-clickable {
		cursor: default;
	}
</style>
