<script lang="ts">
	import { WEB_STATE } from '$lib/redux/store.svelte';
	import { inject } from '@gitbutler/core/context';
	import RegisterInterest from '@gitbutler/shared/interest/RegisterInterest.svelte';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { ORGANIZATION_SERVICE } from '@gitbutler/shared/organizations/organizationService';
	import { getOrganizations } from '@gitbutler/shared/organizations/organizationsPreview.svelte';
	import { PROJECT_SERVICE } from '@gitbutler/shared/organizations/projectService';
	import { projectTable } from '@gitbutler/shared/organizations/projectsSlice';

	import { Button, Modal, SectionCard, chipToasts } from '@gitbutler/ui';
	import type { Project } from '@gitbutler/shared/organizations/types';

	type Props = {
		projectRepositoryId: string;
	};

	const { projectRepositoryId }: Props = $props();

	const webState = inject(WEB_STATE);
	const organizationService = inject(ORGANIZATION_SERVICE);
	const projectService = inject(PROJECT_SERVICE);

	const projectInterest = $derived(projectService.getProjectInterest(projectRepositoryId));
	const project = $derived(
		projectTable.selectors.selectById(webState.projects, projectRepositoryId)
	);

	// Get list of organizations the user belongs to
	const organizations = getOrganizations(webState, organizationService);

	// State to track the currently selected organization
	let selectedOrgSlug = $state<string | null>(null);
	let organizationProjects = $state<Project[]>([]);
	let isLoadingProjects = $state(false);
	let selectedProjectSlug = $state<string | null>(null);
	let newProjectSlug = $state('');
	let currentProjectSlug = $derived(project?.status === 'found' ? project.value.slug : '');
	let isCreatingNew = $state(false);

	// Fetch projects for the selected organization
	async function fetchOrganizationProjects(orgSlug: string) {
		if (!orgSlug) return;

		isLoadingProjects = true;
		try {
			const organization = await organizationService.getOrganizationBySlug(orgSlug);
			if (organization && organization.projectRepositoryIds) {
				const projects: Project[] = [];
				for (const repoId of organization.projectRepositoryIds) {
					const project = await projectService.getProject(repoId);
					if (project) projects.push(project);
				}
				organizationProjects = projects;

				// Default to the project with matching slug if it exists
				const matchingProject = projects.find((p) => p.slug === currentProjectSlug);
				if (matchingProject) {
					selectedProjectSlug = matchingProject.slug;
				} else {
					selectedProjectSlug = null;
				}
			} else {
				organizationProjects = [];
				selectedProjectSlug = null;
			}
		} catch (error) {
			console.error('Failed to fetch organization projects:', error);
			chipToasts.error('Failed to fetch organization projects');
			organizationProjects = [];
		} finally {
			isLoadingProjects = false;
		}
	}

	function selectOrganization(orgSlug: string) {
		selectedOrgSlug = orgSlug;
		fetchOrganizationProjects(orgSlug);
		isCreatingNew = false;
	}

	function toggleCreateNew() {
		isCreatingNew = !isCreatingNew;
		if (isCreatingNew) {
			// Initialize with current project slug as default
			newProjectSlug = currentProjectSlug;
			selectedProjectSlug = null;
		}
	}

	function selectProject(projectSlug: string) {
		selectedProjectSlug = projectSlug;
		isCreatingNew = false;
	}

	async function connectToOrganization(organizationSlug: string) {
		if (project?.status !== 'found') return;

		const projectSlug = isCreatingNew ? newProjectSlug : selectedProjectSlug;

		if (!projectSlug) {
			chipToasts.error('Please select or create a project first');
			return;
		}

		try {
			await projectService.connectProjectToOrganization(
				projectRepositoryId,
				organizationSlug,
				projectSlug
			);
			chipToasts.success('Project connected to organization');
			modal?.close();
		} catch (error) {
			chipToasts.error(
				`Failed to connect project: ${error instanceof Error ? error.message : 'Unknown error'}`
			);
		}
	}

	const title = $derived.by(() => {
		if (project?.status !== 'found') return 'Connect Project';
		return `Connect ${project.value.name} to an Organization`;
	});

	let modal = $state<ReturnType<typeof Modal>>();

	// Expose a show method that can be called from the parent component
	export function show() {
		modal?.show();
	}

	// Reset selection when modal is opened
	$effect(() => {
		if (modal) {
			selectedOrgSlug = null;
			organizationProjects = [];
			selectedProjectSlug = null;
			isCreatingNew = false;
		}
	});
</script>

<Modal bind:this={modal} {title}>
	<RegisterInterest interest={projectInterest} />

	{#if !selectedOrgSlug}
		<!-- Organization Selection Step -->
		{#if organizations.current && organizations.current.length > 0}
			<div class="organizations-list">
				{#each organizations.current as organization, index}
					<Loading loadable={organization}>
						{#snippet children(organization)}
							<SectionCard
								roundedTop={index === 0}
								roundedBottom={index === organizations.current.length - 1}
								orientation="row"
								centerAlign
							>
								<div class="org-info">
									<h5 class="text-15 text-bold">{organization.name || organization.slug}</h5>
									{#if organization.description}
										<p class="description">{organization.description}</p>
									{/if}
								</div>
								<Button style="pop" onclick={() => selectOrganization(organization.slug)}>
									Select
								</Button>
							</SectionCard>
						{/snippet}
					</Loading>
				{/each}
			</div>
		{:else}
			<div class="empty-state">
				<p>You don't belong to any organizations yet.</p>
				<p>Create or join an organization to connect this project.</p>
			</div>
		{/if}
	{:else}
		<!-- Project Selection Step -->
		<div class="selection-header">
			<h4>Select a project in {selectedOrgSlug}</h4>
			<Button style="neutral" onclick={() => (selectedOrgSlug = null)}>Back to Organizations</Button
			>
		</div>

		{#if isLoadingProjects}
			<div class="loading-container">
				<p>Loading projects...</p>
			</div>
		{:else}
			<div class="projects-list">
				{#if organizationProjects.length > 0}
					{#each organizationProjects as orgProject, index}
						<div class={selectedProjectSlug === orgProject.slug ? 'selected' : ''}>
							<SectionCard
								roundedTop={index === 0}
								roundedBottom={index === organizationProjects.length - 1 && !isCreatingNew}
								orientation="row"
								centerAlign
								onclick={() => selectProject(orgProject.slug)}
							>
								<div class="project-info">
									<h5 class="text-15 text-bold">{orgProject.name}</h5>
									{#if orgProject.description}
										<p class="description">{orgProject.description}</p>
									{/if}
									<p class="slug">Slug: {orgProject.slug}</p>
								</div>
								<div class="radio-option">
									<input
										type="radio"
										name="projectSelect"
										checked={selectedProjectSlug === orgProject.slug}
										onclick={() => selectProject(orgProject.slug)}
									/>
								</div>
							</SectionCard>
						</div>
					{/each}
				{/if}

				<!-- Create New Project Option -->
				<div class={isCreatingNew ? 'selected create-new' : 'create-new'}>
					<SectionCard
						roundedTop={organizationProjects.length === 0}
						roundedBottom={true}
						orientation="row"
						centerAlign
						onclick={toggleCreateNew}
					>
						<div class="project-info">
							<h5 class="text-15 text-bold">Create New Project</h5>
							{#if isCreatingNew}
								<div class="new-project-form">
									<label for="newProjectSlug">Project Slug:</label>
									<input
										type="text"
										id="newProjectSlug"
										bind:value={newProjectSlug}
										placeholder="Enter project slug"
										class="form-input"
									/>
								</div>
							{:else}
								<p class="description">Create a new project with this repository</p>
							{/if}
						</div>
						<div class="radio-option">
							<input
								type="radio"
								name="projectSelect"
								checked={isCreatingNew}
								onclick={toggleCreateNew}
							/>
						</div>
					</SectionCard>
				</div>
			</div>

			<div class="action-buttons">
				<Button style="pop" onclick={() => connectToOrganization(selectedOrgSlug || '')}>
					Connect
				</Button>
			</div>
		{/if}
	{/if}
</Modal>

<style lang="postcss">
	.organizations-list,
	.projects-list {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.org-info,
	.project-info {
		flex: 1;
	}

	.description {
		margin-top: 4px;
		color: var(--text-muted, #666);
		font-size: 0.9rem;
	}

	.slug {
		margin-top: 2px;
		color: var(--text-muted, #666);
		font-size: 0.8rem;
	}

	.empty-state {
		padding: 24px 0;
		color: var(--text-muted, #666);
		text-align: center;
	}

	.selection-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 16px;
	}

	.loading-container {
		padding: 32px 0;
		color: var(--text-muted, #666);
		text-align: center;
	}

	.selected {
		border: 2px solid var(--primary, #0366d6);
		background-color: var(--background-hover, #f0f5ff);
	}

	.action-buttons {
		display: flex;
		justify-content: flex-end;
		margin-top: 20px;
	}

	.radio-option {
		display: flex;
		align-items: center;
		justify-content: center;
		margin-left: 10px;
	}

	.new-project-form {
		display: flex;
		flex-direction: column;
		margin-top: 10px;
		gap: 6px;
	}

	.form-input {
		padding: 8px 12px;
		border: 1px solid var(--border-color, #ccc);
		border-radius: 4px;
		font-size: 14px;
	}

	.create-new {
		border-width: 1px;
		border-style: dashed;
	}
</style>
