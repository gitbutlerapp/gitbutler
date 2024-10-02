<script lang="ts">
	import { Project, ProjectService } from '$lib/backend/projects';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import { createGitHostPrServiceStore } from '$lib/gitHost/interface/gitHostPrService';
	import Select from '$lib/select/Select.svelte';
	import SelectItem from '$lib/select/SelectItem.svelte';
	import Section from '$lib/settings/Section.svelte';
	import Spacer from '$lib/shared/Spacer.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';
	import { getContext } from '$lib/utils/context';

	const projectService = getContext(ProjectService);
	const project = getContext(Project);
	const gitHost = getGitHost();
	const prService = createGitHostPrServiceStore(undefined);
	$effect(() => prService.set($gitHost?.prService()));

	let useTemplate = $state(!!project.git_host?.pullRequestTemplatePath);
	let selectedTemplate = $state(project.git_host?.pullRequestTemplatePath ?? '');
	let allAvailableTemplates = $state<{ label: string; value: string }[]>([]);

	$effect(() => {
		if (!project.path) return;
		$prService?.availablePullRequestTemplates(project.path).then((availableTemplates) => {
			if (availableTemplates) {
				allAvailableTemplates = availableTemplates.map((availableTemplate) => {
					const relativePath = availableTemplate.replace(`${project.path}/`, '');
					return {
						label: relativePath,
						value: relativePath
					};
				});
			}
		});
	});

	async function setUsePullRequestTemplate(value: boolean) {
		if (!value) {
			project.git_host.pullRequestTemplatePath = '';
		}
		await projectService.updateProject(project);
	}

	async function setPullRequestTemplatePath(value: string) {
		selectedTemplate = value;
		project.git_host.pullRequestTemplatePath = value;
		await projectService.updateProject(project);
	}
</script>

<Section>
	<div>
		<SectionCard
			roundedBottom={false}
			orientation="row"
			labelFor="use-pull-request-template-boolean"
		>
			<svelte:fragment slot="title">Enable pull request templates</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle
					id="use-pull-request-template-boolean"
					bind:checked={useTemplate}
					on:click={(e) => {
						setUsePullRequestTemplate(
							(e.target as MouseEvent['target'] & { checked: boolean }).checked
						);
					}}
				/>
			</svelte:fragment>
			<svelte:fragment slot="caption">
				If enabled, we will use the path below to set the initial body of any pull requested created
				on this project through GitButler.
			</svelte:fragment>
		</SectionCard>
		<SectionCard roundedTop={false} orientation="row" labelFor="use-pull-request-template-path">
			<svelte:fragment slot="caption">
				<Select
					value={selectedTemplate}
					options={allAvailableTemplates.map(({ label, value }) => ({ label, value }))}
					label="Available Templates"
					wide={true}
					searchable
					disabled={allAvailableTemplates.length === 0}
					onselect={(value) => {
						setPullRequestTemplatePath(value);
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
