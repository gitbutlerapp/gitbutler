<script lang="ts">
	import {
		getAvailablePullRequestTemplates,
		type PullRequestTemplatePaths
	} from '$lib/backend/github';
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
	import Toggle from '$lib/shared/Toggle.svelte';
	import { getContext } from '$lib/utils/context';

	const usePullRequestTemplate = gitHostUsePullRequestTemplate();
	const pullRequestTemplatePath = gitHostPullRequestTemplatePath();

	let selectedTemplate = $state('');
	let allAvailableTemplates = $state<PullRequestTemplatePaths[]>([]);

	const projectService = getContext(ProjectService);
	const id = projectService.getLastOpenedProject();

	$effect(() => {
		if (!id) return;
		getAvailablePullRequestTemplates(id).then((availableTemplates) => {
			if (availableTemplates) {
				allAvailableTemplates = availableTemplates;
			}
		});
	});

	// TODO: Save to project-based settings
	$inspect('SELECTED_TEAMPLTE', selectedTemplate);
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
				<Select
					value={selectedTemplate}
					options={allAvailableTemplates.map(({ label, value }) => ({ label, value }))}
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
