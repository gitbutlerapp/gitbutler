<script lang="ts">
	import { PromptService } from '$lib/ai/promptService';
	import { Project } from '$lib/backend/projects';
	import RadioButton from '$lib/components/RadioButton.svelte';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import Select from './Select.svelte';
	import SelectItem from '$lib/components/SelectItem.svelte';
	import type { Prompts, UserPrompt } from '$lib/ai/types';
	import { getContext } from '$lib/utils/context';
	import type { Persisted } from '$lib/persisted/persisted';

	export let promptUse: 'commits' | 'branches';

	const project = getContext(Project);
	const promptService = getContext(PromptService);

	let prompts: Prompts;
	let selectedPromptId: Persisted<string | undefined>;

	if (promptUse == 'commits') {
		prompts = promptService.commitPrompts;
		selectedPromptId = promptService.selectedCommitPromptId(project.id);
	} else {
		prompts = promptService.branchPrompts;
		selectedPromptId = promptService.selectedBranchPromptId(project.id);
	}

	let userPrompts = prompts.userPrompts;

	let allPrompts: UserPrompt[] = [];

	const defaultId = crypto.randomUUID();

	// let form: HTMLFormElement;

	// function onFormChange(form: HTMLFormElement) {
	// 	const formData = new FormData(form);
	// 	const promptId = formData.get('prompt') as string;

	// 	if (promptId == defaultId) {
	// 		$selectedPromptId = undefined;
	// 	} else {
	// 		$selectedPromptId = promptId;
	// 	}
	// }

	// function initializeForm(form: HTMLFormElement) {
	// 	// If the selectedPromptId is present and cooresponds to a valid prompt
	// 	if ($selectedPromptId && promptService.findPrompt($userPrompts, $selectedPromptId)) {
	// 		form.prompt.value = $selectedPromptId;
	// 	} else {
	// 		form.prompt.value = defaultId;
	// 	}
	// }

	// $: if (form) initializeForm(form);

	$: if ($userPrompts) {
		allPrompts = [
			{ name: 'Default Prompt', id: defaultId, prompt: prompts.defaultPrompt },
			...$userPrompts
		];
		console.log(allPrompts);
	}

	$: if ($selectedPromptId) {
		console.log($selectedPromptId);
	}
</script>

{#if allPrompts.length > 0}
	<Select
		items={allPrompts}
		bind:value={$selectedPromptId}
		itemId="name"
		labelId="name"
		disabled={allPrompts.length == 1}
		wide={true}
		label={promptUse == 'commits' ? 'Commit Message' : 'Branch Name'}
	>
		<SelectItem slot="template" let:item let:selected {selected}>
			{item.name}
		</SelectItem>
	</Select>
{/if}

<!-- <form bind:this={form} on:change={(e) => onFormChange(e.currentTarget)}>
		<SectionCard roundedBottom={false} labelFor={defaultId} orientation="row">
			<svelte:fragment slot="title">Default Prompt</svelte:fragment>
			<svelte:fragment slot="actions">
				<RadioButton name="prompt" id={defaultId} value={defaultId} />
			</svelte:fragment>
		</SectionCard>
		{#each $userPrompts as userPrompt, index}
			{@const disabled = promptService.promptMissingContent(userPrompt.prompt)}
			<SectionCard
				roundedTop={false}
				roundedBottom={index + 1 == $userPrompts.length}
				labelFor={userPrompt.id}
				orientation="row"
				{disabled}
			>
				<svelte:fragment slot="title">{userPrompt.name}</svelte:fragment>

				<svelte:fragment slot="caption"
					>{#if disabled}
						This prompt contains blank messages, please update the prompt in order to
						select it.
					{/if}</svelte:fragment
				>

				<svelte:fragment slot="actions">
					<RadioButton name="prompt" id={userPrompt.id} value={userPrompt.id} />
				</svelte:fragment>
			</SectionCard>
		{/each}
	</form> -->
