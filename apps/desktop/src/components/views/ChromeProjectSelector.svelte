<script lang="ts">
	import { goto } from "$app/navigation";
	import { handleAddProjectOutcome } from "$lib/project/project";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { projectPath } from "$lib/routes/routes.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Button, Icon, OptionsGroup, Select, SelectItem, TestId } from "@gitbutler/ui";

	type Props = {
		projectId: string;
		projectTitle: string;
	};

	const { projectId, projectTitle }: Props = $props();

	const projectsService = inject(PROJECTS_SERVICE);
	const serverCapabilitiesQuery = $derived(projectsService.serverCapabilities());
	const canAddProjects = $derived(serverCapabilitiesQuery.response?.canAddProjects ?? true);
	const projects = $derived(projectsService.projects());

	const mappedProjects = $derived(
		projects.response?.map((project) => ({
			value: project.id,
			label: project.title,
		})) || [],
	);

	let newProjectLoading = $state(false);
	let projectSelectorOpen = $state(false);
</script>

<div class="chrome-project-selector">
	<Select
		searchable
		value={projectId}
		options={mappedProjects}
		loading={newProjectLoading}
		disabled={newProjectLoading}
		onselect={(value: string, modifiers?) => {
			if (modifiers?.meta) {
				projectsService.openProjectInNewWindow(value);
			} else {
				goto(projectPath(value));
			}
		}}
		ontoggle={(isOpen) => (projectSelectorOpen = isOpen)}
		popupAlign="center"
		customWidth={280}
	>
		{#snippet customSelectButton()}
			<Button
				testId={TestId.ChromeHeaderProjectSelector}
				reversedDirection
				width="auto"
				kind="outline"
				isDropdown
				dropdownOpen={projectSelectorOpen}
				class="project-selector-btn"
			>
				{#snippet custom()}
					<div class="project-selector-btn__content">
						<Icon name="repo" color="var(--text-2)" />
						<span class="text-12 text-bold">{projectTitle}</span>
					</div>
				{/snippet}
			</Button>
		{/snippet}

		{#snippet itemSnippet({ item, highlighted })}
			<SelectItem selected={item.value === projectId} {highlighted}>
				{item.label}
			</SelectItem>
		{/snippet}

		<OptionsGroup>
			{#if canAddProjects}
				<SelectItem
					icon="plus"
					testId={TestId.ChromeHeaderProjectSelectorAddLocalProject}
					loading={newProjectLoading}
					onClick={async () => {
						newProjectLoading = true;
						try {
							const outcome = await projectsService.addProject();
							if (!outcome) {
								newProjectLoading = false;
								return;
							}

							handleAddProjectOutcome(outcome, (project) => goto(projectPath(project.id)));
						} finally {
							newProjectLoading = false;
						}
					}}
				>
					Add local repository
				</SelectItem>
			{/if}
			<SelectItem
				icon="clone"
				onClick={() => {
					goto("/onboarding/clone");
				}}
			>
				Clone repository
			</SelectItem>
		</OptionsGroup>
	</Select>
</div>

<style>
	.chrome-project-selector {
		display: flex;
		min-width: 0;
	}

	:global(.project-selector-btn) {
		max-width: min(320px, calc(100vw - 120px));
	}

	.project-selector-btn__content {
		display: flex;
		align-items: center;
		min-width: 0;
		padding-right: 2px;
		gap: 6px;
		text-wrap: nowrap;
	}

	.project-selector-btn__content span {
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
