<script lang="ts">
	import { ProjectService } from '$lib/backend/projects';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import {
		gitHostPullRequestTemplatePath,
		gitHostUsePullRequestTemplate
	} from '$lib/config/config';
	import Select from '$lib/select/Select.svelte';
	import SelectItem from '$lib/select/SelectItem.svelte';
	import Section from '$lib/settings/Section.svelte';
	import Spacer from '$lib/shared/Spacer.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';
	import { getContext } from '$lib/utils/context';
	// import { join } from '@tauri-apps/api/path';
	// import { readDir, type DirEntry } from '@tauri-apps/plugin-fs';
	import { type DirEntry } from '@tauri-apps/plugin-fs';
	import { onMount } from 'svelte';
	// import { get } from 'svelte/store';

	const usePullRequestTemplate = gitHostUsePullRequestTemplate();
	const pullRequestTemplatePath = gitHostPullRequestTemplatePath();

	let selectedTemplate = $state('');
	let allAvailableTemplates = $state<DirEntry[]>([]);

	const projectService = getContext(ProjectService);
	// const currentProjectId = get(projectService.persistedId);

	onMount(async () => {
		const availableTemplates = await projectService.getAvailablePullRequestTemplates();
		allAvailableTemplates = availableTemplates;

		// console.log('project1', currentProjectId);
		// const currentProject = await projectService.getProject(currentProjectId);
		// console.log('currentProject', currentProject);
		// const targetPath = await join(currentProject.path, '.github');
		// readDir(targetPath).then((files) => {
		// 	console.log('GITHUB FILES', files);
		// 	files.forEach((file) => {
		// 		allAvailableTemplates.push(file);
		// 	});
		// });
	});
</script>

<Section>
	<svelte:fragment slot="title">Pull Request Template</svelte:fragment>
	<svelte:fragment slot="description">
		Use Pull Request template when creating a Pull Requests.
	</svelte:fragment>

	<div>
		<SectionCard
			roundedBottom={false}
			orientation="row"
			labelFor="use-pull-request-template-boolean"
		>
			<svelte:fragment slot="title">Enable Pull Request Templates</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle
					id="use-pull-request-template-boolean"
					value="false"
					bind:checked={$usePullRequestTemplate}
				/>
			</svelte:fragment>
			<svelte:fragment slot="caption">
				If enabled, we will use the path below to set the initial body of any pull requested created
				on this project through GitButler.
			</svelte:fragment>
		</SectionCard>
		<SectionCard roundedTop={false} orientation="row" labelFor="use-pull-request-template-path">
			<svelte:fragment slot="caption">
				<form>
					<fieldset class="fields-wrapper">
						<TextBox
							label="Pull request template path"
							id="use-pull-request-template-path"
							bind:value={$pullRequestTemplatePath}
							placeholder=".github/pull_request_template.md"
						/>
					</fieldset>
				</form>

				<Select
					value={selectedTemplate}
					options={allAvailableTemplates.map((p) => ({ label: p.name, value: p.name }))}
					label="Available Templates"
					wide={true}
					searchable
					disabled={false}
					onselect={(value) => {
						selectedTemplate = value;
					}}
				>
					{#snippet itemSnippet({ item, highlighted })}
						<SelectItem selected={item.value === selectedTemplate} {highlighted}>
							{item.label}
						</SelectItem>
					{/snippet}
				</Select>
			</svelte:fragment>
		</SectionCard>
	</div>
</Section>
<Spacer />
