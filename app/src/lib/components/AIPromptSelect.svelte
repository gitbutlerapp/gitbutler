<script lang="ts">
	import Select from '../shared/Select.svelte';
	import { PromptService } from '$lib/ai/promptService';
	import { Project } from '$lib/backend/projects';
	import SelectItem from '$lib/shared/SelectItem.svelte';
	import { getContext } from '$lib/utils/context';
	import type { Prompts, UserPrompt } from '$lib/ai/types';
	import type { Persisted } from '$lib/persisted/persisted';

	export let promptUse: 'commits' | 'branches';

	const project = getContext(Project);
	const promptService = getContext(PromptService);

	let prompts: Prompts;
	let selectedPromptId: Persisted<string | undefined>;

	if (promptUse === 'commits') {
		prompts = promptService.commitPrompts;
		selectedPromptId = promptService.selectedCommitPromptId(project.id);
	} else {
		prompts = promptService.branchPrompts;
		selectedPromptId = promptService.selectedBranchPromptId(project.id);
	}

	let userPrompts = prompts.userPrompts;

	let allPrompts: UserPrompt[] = [];

	const defaultId = crypto.randomUUID();

	function setAllPrompts(userPrompts: UserPrompt[]) {
		allPrompts = [
			{ name: 'Default Prompt', id: defaultId, prompt: prompts.defaultPrompt },
			...userPrompts
		];
	}

	$: setAllPrompts($userPrompts);

	$: if (!$selectedPromptId || !promptService.findPrompt(allPrompts, $selectedPromptId)) {
		$selectedPromptId = defaultId;
	}
</script>

<Select
	items={allPrompts}
	bind:selectedItemId={$selectedPromptId}
	itemId="id"
	labelId="name"
	disabled={allPrompts.length === 1}
	wide={true}
	label={promptUse === 'commits' ? 'Commit message' : 'Branch name'}
>
	<SelectItem slot="template" let:item let:selected {selected} let:highlighted {highlighted}>
		{item.name}
		{highlighted}
	</SelectItem>
</Select>
