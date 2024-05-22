<script lang="ts">
	import { PromptService } from '$lib/ai/promptService';
	import { Project } from '$lib/backend/projects';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import { getContext } from '$lib/utils/context';
	import type { Prompts } from '$lib/ai/types';

	export let promptUse: 'commits' | 'branches';

	const project = getContext(Project);
	const promptService = getContext(PromptService);

	let prompts: Prompts;

	if (promptUse == 'commits') {
		prompts = promptService.commitPrompts;
	} else {
		prompts = promptService.branchPrompts;
	}

	console.log(promptService);
	console.log(prompts);

	$: userPrompts = prompts.userPrompts;
</script>

<h3 class="text-base-15 text-bold">
	{promptUse == 'commits' ? 'Commit Prompts' : 'Branch Prompts'}
</h3>

{#each $userPrompts as userPrompt, index}
	<SectionCard roundedTop={index == 0} roundedBottom={index + 1 == $userPrompts.length}
	></SectionCard>
{/each}
