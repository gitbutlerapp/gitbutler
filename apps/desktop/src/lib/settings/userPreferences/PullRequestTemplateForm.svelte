<script lang="ts">
	import notFoundSvg from '$lib/assets/empty-state/not-found.svg?raw';
	import { Project, ProjectsService } from '$lib/backend/projects';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import { createGitHostPrServiceStore } from '$lib/gitHost/interface/gitHostPrService';
	import Select from '$lib/select/Select.svelte';
	import SelectItem from '$lib/select/SelectItem.svelte';
	import Section from '$lib/settings/Section.svelte';
	import Link from '$lib/shared/Link.svelte';
	import Spacer from '$lib/shared/Spacer.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import EmptyStatePlaceholder from '@gitbutler/ui/EmptyStatePlaceholder.svelte';

	const projectsService = getContext(ProjectsService);
	const project = getContext(Project);
	const gitHost = getGitHost();
	const prService = createGitHostPrServiceStore(undefined);
	$effect(() => prService.set($gitHost?.prService()));

	let useTemplate = $state(!!project.git_host?.pullRequestTemplatePath);
	let selectedTemplate = $state(project.git_host?.pullRequestTemplatePath ?? '');
	let allAvailableTemplates = $state<{ label: string; value: string }[]>([]);
	let isTemplatesAvailable = $state(false);

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

				isTemplatesAvailable = allAvailableTemplates.length > 0;
			}
		});
	});

	async function setUsePullRequestTemplate(value: boolean) {
		if (!value) {
			project.git_host.pullRequestTemplatePath = '';
		}
		await projectsService.updateProject(project);
	}

	async function setPullRequestTemplatePath(value: string) {
		selectedTemplate = value;
		project.git_host.pullRequestTemplatePath = value;
		await projectsService.updateProject(project);
	}
</script>

<Section>
	<div class="stack-v">
		<SectionCard
			roundedBottom={!useTemplate}
			orientation="row"
			labelFor="use-pull-request-template-boolean"
		>
			<svelte:fragment slot="title">Pull request templates</svelte:fragment>
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

		{#if useTemplate}
			<SectionCard roundedTop={false} orientation="row">
				{#if isTemplatesAvailable}
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
				{:else}
					<EmptyStatePlaceholder image={notFoundSvg} topBottomPadding={20}>
						{#snippet caption()}
							No templates found in the project
							<span class="text-11">
								<Link
									href="https://docs.github.com/en/communities/using-templates-to-encourage-useful-issues-and-pull-requests/creating-a-pull-request-template-for-your-repository"
									>How to create a template</Link
								></span
							>
						{/snippet}
					</EmptyStatePlaceholder>
				{/if}
			</SectionCard>
		{/if}
	</div>
</Section>
<Spacer />
