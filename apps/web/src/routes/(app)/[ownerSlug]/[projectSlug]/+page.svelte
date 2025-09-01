<script lang="ts">
	import { goto } from '$app/navigation';
	import ProjectConnectModal from '$lib/components/ProjectConnectModal.svelte';
	import ReviewsSection from '$lib/components/ReviewsSection.svelte';
	import { featureShowProjectPage } from '$lib/featureFlags';
	import { getTimeSince } from '$lib/utils/dateUtils';
	import { inject } from '@gitbutler/core/context';
	import PermissionsSelector from '@gitbutler/shared/organizations/PermissionsSelector.svelte';
	import { PROJECT_SERVICE } from '@gitbutler/shared/organizations/projectService';
	import {
		WEB_ROUTES_SERVICE,
		type ProjectParameters
	} from '@gitbutler/shared/routing/webRoutes.svelte';

	import { AsyncButton, Button, Markdown, Modal, chipToasts } from '@gitbutler/ui';

	interface Props {
		data: ProjectParameters;
	}

	let { data }: Props = $props();
	const projectService = inject(PROJECT_SERVICE);
	const routes = inject(WEB_ROUTES_SERVICE);

	$effect(() => {
		if (!$featureShowProjectPage) {
			goto(routes.homePath());
		}
	});

	// Store project data in a reactive variable
	let projectData = $state<any>(null);

	// Create a promise for the project data
	const projectSlug = `${data.ownerSlug}/${data.projectSlug}`;
	const projectPromise = projectService.getProjectBySlug(projectSlug).then((result) => {
		if (result) {
			projectData = result;
		}
		return result;
	});

	// Create a promise for the patch stacks data
	let patchStacksData = $state<any[]>([]);
	const patchStacksPromise = projectService.getProjectPatchStacks(projectSlug).then((result) => {
		if (result) {
			patchStacksData = result;
		}
		return result;
	});

	// Start editing the README
	function startEditingReadme(currentReadme: string | undefined) {
		readmeContent = currentReadme || '';
		editingReadme = true;
	}

	// Cancel editing the README
	function cancelEditingReadme() {
		editingReadme = false;
	}

	// Save the README
	async function saveReadme(repositoryId: string) {
		try {
			isSavingReadme = true;
			// Use a type assertion since readme isn't part of UpdateParams
			await projectService.updateProject(repositoryId, { readme: readmeContent } as any);

			// Update the local project data with the new README
			projectData = {
				...projectData,
				readme: readmeContent
			};

			editingReadme = false;
			chipToasts.success('README updated successfully');
		} catch (error) {
			chipToasts.error(
				`Failed to update README: ${error instanceof Error ? error.message : 'Unknown error'}`
			);
		} finally {
			isSavingReadme = false;
		}
	}

	// README editing state
	let editingReadme = $state(false);
	let readmeContent = $state('');
	let isSavingReadme = $state(false);

	// Project edit state and modal reference
	let editProjectModal = $state<ReturnType<typeof Modal> | undefined>(undefined);
	let editedName = $state('');
	let editedSlug = $state('');
	let editedDescription = $state('');
	let isUpdatingProject = $state(false);

	// Open edit project modal
	function openEditProjectModal() {
		editedName = projectData.name || '';
		editedSlug = projectData.slug || '';
		editedDescription = projectData.description || '';
		editProjectModal?.show();
	}

	// Save project edits
	async function saveProjectEdits(repositoryId: string) {
		try {
			isUpdatingProject = true;

			const updateParams = {
				name: editedName,
				slug: editedSlug,
				description: editedDescription
			};

			const updatedProject = await projectService.updateProject(repositoryId, updateParams);

			// Update the local project data
			projectData = {
				...projectData,
				...updatedProject
			};

			editProjectModal?.close();
			chipToasts.success('Project updated successfully');

			// If the slug changed, redirect to the new URL
			if (editedSlug !== data.projectSlug) {
				goto(
					routes.projectPath({
						ownerSlug: data.ownerSlug,
						projectSlug: editedSlug
					})
				);
			}
		} catch (error) {
			chipToasts.error(`Failed to update project`);
			console.error(
				`Failed to update project: ${error instanceof Error ? error.message : 'Unknown error'}`
			);
		} finally {
			isUpdatingProject = false;
		}
	}

	async function deleteProject(repositoryId: string) {
		if (!confirm('Are you sure you want to delete this project?')) {
			return;
		}

		await projectService.deleteProject(repositoryId);
		goto(routes.projectsPath());
	}

	async function handleDisconnectFromParent() {
		if (!confirm('Are you sure you want to disconnect this project from its parent?')) {
			return;
		}

		try {
			await projectService.disconnectProject(projectData.repositoryId);
			projectData = {
				...projectData,
				parentProject: undefined,
				parentProjectRepositoryId: undefined
			};
			chipToasts.success('Project unlinked from parent');
		} catch (error) {
			chipToasts.error(`Failed to unlink project`);
			console.error(
				`Failed to unlink project: ${error instanceof Error ? error.message : 'Unknown error'}`
			);
		}
	}

	let connectModal = $state<ReturnType<typeof ProjectConnectModal> | undefined>(undefined);
</script>

{#await projectPromise}
	<div class="loading-container">
		<p>Loading project...</p>
	</div>
{:then _projectData}
	{#if _projectData}
		<div class="project-page">
			<header class="project-header">
				<div class="breadcrumb">
					<a href={routes.projectPath({ ownerSlug: data.ownerSlug, projectSlug: '' })}>
						{data.ownerSlug}
					</a>
					<span>/</span>
					<h1>{data.projectSlug}</h1>
				</div>
				{#if projectData.parentProject}
					<div class="parent-project-info">
						<span class="label">Parent Project:</span>
						<a
							href={routes.projectPath({
								ownerSlug: projectData.parentProject.owner,
								projectSlug: projectData.parentProject.slug
							})}
						>
							{projectData.parentProject.owner}/{projectData.parentProject.slug}
						</a>
					</div>
				{/if}
			</header>

			<div class="project-grid">
				<div class="main-content">
					<!-- Reviews section using the ReviewsSection component -->
					{#await patchStacksPromise}
						<ReviewsSection reviews={[]} status="loading" sectionTitle="Active Reviews" />
					{:then _}
						<ReviewsSection
							reviews={patchStacksData || []}
							status={patchStacksData && patchStacksData.length > 0 ? 'found' : 'not-found'}
							sectionTitle="Active Reviews"
							allReviewsUrl={routes.projectReviewPath(data)}
							reviewsCount={projectData.activeReviewsCount || 0}
						/>
					{:catch error}
						<ReviewsSection reviews={[]} status="error" sectionTitle="Active Reviews" />
						<div class="error-text">
							Error loading reviews: {error.message || 'Unknown error'}
						</div>
					{/await}

					<section class="card">
						<div class="readme-header">
							<h2 class="card-title">README</h2>
							{#if projectData.permissions?.canWrite}
								<div class="readme-actions">
									{#if editingReadme}
										<AsyncButton
											style="pop"
											action={() => saveReadme(projectData.repositoryId)}
											disabled={isSavingReadme}
										>
											Save
										</AsyncButton>
										<Button
											type="button"
											style="neutral"
											onclick={cancelEditingReadme}
											disabled={isSavingReadme}
										>
											Cancel
										</Button>
									{:else}
										<Button
											type="button"
											style="neutral"
											onclick={() => startEditingReadme((projectData as any).readme)}
										>
											Edit README
										</Button>
									{/if}
								</div>
							{/if}
						</div>
						<div class="card-content readme">
							{#if editingReadme}
								<textarea
									bind:value={readmeContent}
									class="readme-editor"
									rows="15"
									placeholder="Enter markdown content for the README..."
									disabled={isSavingReadme}
								></textarea>
								<div class="readme-preview">
									<h3 class="preview-title">Preview</h3>
									<Markdown content={readmeContent} />
								</div>
							{:else if (projectData as any).readme}
								<Markdown content={(projectData as any).readme} />
							{:else}
								<div class="no-readme">
									{#if projectData.permissions?.canWrite}
										<p>No README available for this project. Click "Edit README" to create one.</p>
									{:else}
										<p>No README available for this project.</p>
									{/if}
								</div>
							{/if}
						</div>
					</section>
				</div>

				<div class="sidebar">
					<section class="card">
						<div class="card-header">
							<h2 class="card-title">Project Details</h2>
							{#if projectData.permissions?.canWrite}
								<Button
									type="button"
									style="pop"
									onclick={openEditProjectModal}
									class="edit-project-btn"
								>
									Edit Project
								</Button>
							{/if}
						</div>
						<div class="card-content">
							{#if projectData.name}
								<h3 class="sidebar-section-title">Name</h3>
								<p class="description">
									{projectData.name}
								</p>
							{/if}
							{#if projectData.description}
								<h3 class="sidebar-section-title">Description</h3>
								<p class="description">
									{projectData.description}
								</p>
							{/if}

							<h3 class="sidebar-section-title">Last Updated</h3>
							<p class="description">{getTimeSince(projectData.updatedAt)}</p>

							{#if projectData.lastPushedAt}
								<div class="meta-info">
									<div class="meta-item clone-url-container">
										<h3 class="sidebar-section-title">Clone URL</h3>
										<div class="clone-url">
											<code>{projectData.codeGitUrl}</code>
											<Button
												type="button"
												style="pop"
												onclick={() => {
													navigator.clipboard.writeText(projectData.codeGitUrl);
													chipToasts.success('copied to clipboard');
												}}
											>
												Copy
											</Button>
										</div>
									</div>
								</div>
							{/if}
						</div>
					</section>

					{#if projectData.parentProject}
						<section class="card">
							<h2 class="card-title">Parent Project</h2>
							<div class="card-content">
								<div class="parent-project-info-card">
									<p>
										This project is linked to a parent project:
										<a
											href={routes.projectPath({
												ownerSlug: projectData.parentProject?.owner || data.ownerSlug,
												projectSlug: projectData.parentProject?.slug || ''
											})}
										>
											{projectData.parentProject?.owner || data.ownerSlug}/{projectData
												.parentProject?.slug || projectData.parentProjectRepositoryId}
										</a>
									</p>

									{#if projectData.permissions?.canWrite}
										<Button style="error" onclick={handleDisconnectFromParent}>
											Disconnect from Parent
										</Button>
									{/if}
								</div>
							</div>
						</section>
					{:else if projectData.ownerType === 'user' && projectData.permissions?.canWrite}
						<section class="card">
							<h2 class="card-title">Connect to Organization</h2>
							<div class="card-content">
								<div class="connect-org-card">
									<p>Connect this project to an organization to enable team collaboration.</p>
									<Button style="pop" onclick={() => connectModal?.show()}>
										Connect to Organization
									</Button>
								</div>
							</div>
						</section>

						<ProjectConnectModal
							bind:this={connectModal}
							projectRepositoryId={projectData.repositoryId}
						/>
					{/if}

					{#if projectData.permissions?.canWrite}
						<section class="card">
							<h2 class="card-title">Permissions</h2>
							<div class="card-content gap-2">
								<p>This project is <b>{projectData.permissions.shareLevel}</b></p>
								<PermissionsSelector repositoryId={projectData.repositoryId} />
							</div>
						</section>

						<section class="card danger-zone">
							<h2 class="card-title danger-title">Danger Zone</h2>
							<div class="card-content">
								<AsyncButton
									style="error"
									action={async () => await deleteProject(projectData.repositoryId)}
								>
									Delete Project
								</AsyncButton>
							</div>
						</section>
					{/if}
				</div>
			</div>
		</div>

		<!-- Edit Project Modal -->
		<Modal
			bind:this={editProjectModal}
			title="Edit Project"
			onClose={() => {
				isUpdatingProject = false;
			}}
		>
			<form class="edit-project-form">
				<div class="form-group">
					<label for="project-name">Project Name</label>
					<input
						id="project-name"
						type="text"
						bind:value={editedName}
						placeholder="Project name"
						required
						disabled={isUpdatingProject}
					/>
				</div>

				<div class="form-group">
					<label for="project-slug">Project Slug</label>
					<input
						id="project-slug"
						type="text"
						bind:value={editedSlug}
						placeholder="project-slug"
						required
						disabled={isUpdatingProject}
						pattern="[a-z0-9-]+"
						title="Lowercase letters, numbers, and hyphens only"
					/>
					<small>Only lowercase letters, numbers, and hyphens are allowed</small>
				</div>

				<div class="form-group">
					<label for="project-description">Description</label>
					<textarea
						id="project-description"
						bind:value={editedDescription}
						placeholder="Project description"
						rows="4"
						disabled={isUpdatingProject}
					></textarea>
				</div>

				<div class="form-actions">
					<Button
						type="button"
						style="neutral"
						onclick={() => editProjectModal?.close()}
						disabled={isUpdatingProject}
					>
						Cancel
					</Button>
					<AsyncButton
						style="pop"
						action={() => saveProjectEdits(projectData.repositoryId)}
						disabled={isUpdatingProject}
					>
						Save Changes
					</AsyncButton>
				</div>
			</form>
		</Modal>
	{:else}
		<div class="error-message">
			<h2>Project Not Found</h2>
			<p>The project you requested could not be found. Please check the URL and try again.</p>
			<Button onclick={() => goto(routes.projectsPath())}>Return to Projects</Button>
		</div>
	{/if}
{:catch error}
	<div class="error-message">
		<h2>Error Loading Project</h2>
		<p>There was a problem loading the project: {error.message || 'Unknown error'}</p>
		<Button onclick={() => goto(routes.projectsPath())}>Return to Projects</Button>
	</div>
{/await}

<style lang="postcss">
	.loading-container {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 200px;
		color: var(--text-muted, #666);
		font-size: 1.2rem;
	}

	.error-text {
		padding: 1rem 0;
		color: var(--error, #dc3545);
		text-align: center;
	}

	.error-message {
		max-width: 600px;
		margin: 2rem auto;
		padding: 2rem;
		border: 1px solid var(--border-color, #eaeaea);
		border-radius: 8px;
		background-color: var(--background, #fff);
		text-align: center;

		h2 {
			margin: 0 0 1rem;
			color: var(--error, #dc3545);
		}

		p {
			margin-bottom: 1.5rem;
		}
	}

	.project-header {
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		margin-bottom: 2rem;
	}

	.parent-project-info {
		display: flex;
		align-items: center;
		margin-top: 10px;
		gap: 0.5rem;
		color: var(--text-muted, #666);
		font-size: 13px;
	}

	.breadcrumb {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-size: 1.2rem;

		a {
			color: var(--text-muted, #666);
			text-decoration: none;

			&:hover {
				text-decoration: underline;
			}
		}

		h1 {
			margin: 0;
		}
	}

	.project-grid {
		display: grid;
		grid-template-columns: 2fr 1fr;
		gap: 1.5rem;
	}

	.main-content {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.sidebar {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.card {
		overflow: hidden;
		border: 1px solid color(srgb 0.831373 0.815686 0.807843);
		border-radius: 8px;
		background-color: white;
	}

	.card-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-right: 15px;
		border-bottom: 1px solid color(srgb 0.831373 0.815686 0.807843);
		background-color: #f3f3f2;
	}

	.card-title {
		margin: 0;
		padding: 12px 15px;
		border-bottom: 1px solid color(srgb 0.831373 0.815686 0.807843);
		background-color: #f3f3f2;
		color: color(srgb 0.52549 0.494118 0.47451);
		font-size: 0.8em;
	}

	.card-header .card-title {
		border-bottom: none;
	}

	.readme-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		border-bottom: 1px solid color(srgb 0.831373 0.815686 0.807843);
		background-color: #f3f3f2;
	}

	.readme-header .card-title {
		border-bottom: none;
	}

	.readme-actions {
		display: flex;
		padding-right: 15px;
		gap: 0.5rem;
	}

	.readme-editor {
		width: 100%;
		min-height: 200px;
		margin-bottom: 1rem;
		padding: 0.75rem;
		border: 1px solid var(--border-color, #eaeaea);
		border-radius: 4px;
		font-family: var(--fontfamily-mono);
		resize: vertical;
	}

	.readme-preview {
		margin-top: 1rem;
		padding-top: 1rem;
		border-top: 1px solid var(--border-color, #eaeaea);
	}

	.preview-title {
		margin: 0 0 0.75rem 0;
		color: var(--text-muted, #666);
		font-size: 1rem;
	}

	.card-content {
		padding: 1rem;
	}

	.meta-info {
		margin-top: 1.5rem;
	}

	.meta-item {
		display: flex;
		align-items: flex-start;
		margin-bottom: 1rem;
	}

	.sidebar-section-title {
		margin: 0 0 0.5rem 0;
		color: var(--text-muted, #666);
		font-size: 1rem;
	}

	.description {
		margin-bottom: 1.5rem;
		line-height: 1.4;
	}

	.clone-url-container {
		display: block;
	}

	.clone-url {
		display: flex;
		align-items: center;
		width: 100%;
		gap: 0.5rem;

		code {
			flex: 1;
			padding: 0.25rem 0.5rem;
			overflow: hidden;
			border-radius: 4px;
			background: var(--background-alt, #f5f5f5);
			font-family: var(--fontfamily-mono);
			text-overflow: ellipsis;
		}
	}

	.parent-project-info-card {
		display: flex;
		flex-direction: column;
		gap: 1rem;

		p {
			margin: 0;
			line-height: 1.4;
		}

		a {
			display: inline-block;
			margin-top: 0.25rem;
			color: var(--clr-core-pop-50);
			font-weight: 500;

			&:hover {
				text-decoration: underline;
			}
		}
	}

	.readme {
		font-size: 0.95rem;
		line-height: 1.5;
	}

	.danger-title {
		color: var(--error, #dc3545);
	}

	.danger-zone {
		margin-top: auto;
	}

	.gap-2 {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.no-readme {
		padding: 0.5rem 0;
		color: #718096;
		text-align: center;
	}

	.connect-org-card {
		display: flex;
		flex-direction: column;
		gap: 1rem;

		p {
			margin: 0;
			line-height: 1.4;
		}
	}

	.edit-project-form {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.form-group {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.form-group label {
		font-weight: 500;
	}

	.form-group input,
	.form-group textarea {
		padding: 0.5rem;
		border: 1px solid color(srgb 0.831373 0.815686 0.807843);
		border-radius: 4px;
		font-size: 14px;
	}

	.form-group small {
		color: var(--text-muted, #666);
		font-size: 12px;
	}

	.form-actions {
		display: flex;
		justify-content: flex-end;
		margin-top: 1rem;
		gap: 0.5rem;
	}

	@media (max-width: 768px) {
		.project-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
