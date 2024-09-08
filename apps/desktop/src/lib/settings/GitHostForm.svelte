<script lang="ts">
	import {
		getAvailablePullRequestTemplates,
		type PullRequestTemplatePaths
	} from '$lib/backend/github';
	import { Project, ProjectService } from '$lib/backend/projects';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import Select from '$lib/select/Select.svelte';
	import SelectItem from '$lib/select/SelectItem.svelte';
	import Section from '$lib/settings/Section.svelte';
	import Spacer from '$lib/shared/Spacer.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';
	import { getContext } from '$lib/utils/context';

	const projectService = getContext(ProjectService);
	const project = getContext(Project);

	let useTemplate = $state(project.git_host?.use_pull_request_template ?? false);
	let selectedTemplate = $state(project.git_host?.pull_request_template_path ?? '');
	let allAvailableTemplates = $state<PullRequestTemplatePaths[]>([]);

	$effect(() => {
		if (!project.path) return;
		getAvailablePullRequestTemplates(project.path).then((availableTemplates) => {
			if (availableTemplates) {
				allAvailableTemplates = availableTemplates;
			}
		});
	});

	// TODO: investigate if theres a better wayt o get old clients up to speed with
	// new preferences keys
	async function updateExistingProjects() {
		if (!project.git_host) {
			project.git_host = {
				host_type: 'github',
				use_pull_request_template: false,
				pull_request_template_path: ''
			};
			await projectService.updateProject(project);
		}
	}

	async function setUsePullRequestTemplate(value: boolean) {
		await updateExistingProjects();
		// project[gitHost.type].use_pull_request_template = value;
		project.git_host.use_pull_request_template = value;
		await projectService.updateProject(project);
	}

	async function setPullRequestTemplatePath(value: string) {
		selectedTemplate = value;
		await updateExistingProjects();

		// project[gitHost.type].pull_request_template_path = value;
		project.git_host.pull_request_template_path = value;
		await projectService.updateProject(project);
	}
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
					bind:checked={useTemplate}
					on:click={(e) => {
						setUsePullRequestTemplate((e.target as MouseEvent['target'] & { checked: boolean }).checked);
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
					disabled={false}
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
