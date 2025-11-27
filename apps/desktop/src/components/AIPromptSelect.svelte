<script lang="ts">
	import { PROMPT_SERVICE } from '$lib/ai/promptService';
	import { inject } from '@gitbutler/core/context';
	import { Select, SelectItem } from '@gitbutler/ui';
	import { onMount } from 'svelte';
	import type { Prompts, UserPrompt } from '$lib/ai/types';
	import type { Persisted } from '@gitbutler/shared/persisted';

	type Props = {
		projectId: string;
		promptUse: 'commits' | 'branches';
	};

	const { projectId, promptUse }: Props = $props();

	const promptService = inject(PROMPT_SERVICE);

	let prompts: Prompts;
	let selectedPromptId = $state<Persisted<string | undefined>>();

	if (promptUse === 'commits') {
		prompts = promptService.commitPrompts;
		selectedPromptId = promptService.selectedCommitPromptId(projectId);
	} else {
		prompts = promptService.branchPrompts;
		selectedPromptId = promptService.selectedBranchPromptId(projectId);
	}

	let userPrompts = prompts.userPrompts;
	let allPrompts: UserPrompt[] = $state([]);

	const defaultId = crypto.randomUUID();

	function setAllPrompts(userPrompts: UserPrompt[]) {
		allPrompts = [
			{ name: 'Default Prompt', id: defaultId, prompt: prompts.defaultPrompt },
			...userPrompts
		];
	}

	onMount(() => {
		setAllPrompts($userPrompts);
	});

	$effect(() => {
		if (!$selectedPromptId || !promptService.findPrompt(allPrompts, $selectedPromptId)) {
			$selectedPromptId = defaultId;
		}
	});
</script>

<Select
	value={$selectedPromptId}
	options={allPrompts.map((p) => ({ label: p.name, value: p.id }))}
	label={promptUse === 'commits' ? 'Commit message' : 'Branch name'}
	wide={true}
	searchable
	disabled={allPrompts.length === 1}
	onselect={(value) => {
		$selectedPromptId = value;
	}}
>
	{#snippet itemSnippet({ item, highlighted })}
		<SelectItem selected={item.value === $selectedPromptId} {highlighted}>
			{item.label}
		</SelectItem>
	{/snippet}
</Select>
