<script lang="ts">
	import ProjectsSection from '$lib/components/ProjectsSection.svelte';
	import ReviewsSection from '$lib/components/ReviewsSection.svelte';
	import { UserService } from '$lib/user/userService';
	import AsyncButton from '@gitbutler/ui/AsyncButton.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Markdown from '@gitbutler/ui/markdown/Markdown.svelte';
	import toasts from '@gitbutler/ui/toasts';
	import { getContext } from 'svelte';
	import { get } from 'svelte/store';
	import type { ExtendedUser } from '$lib/owner/types';

	interface Props {
		user: ExtendedUser;
		ownerSlug: string;
	}

	let { user, ownerSlug }: Props = $props();

	const userService = getContext(UserService) as UserService;
	const currentUser = userService.user;
	const isCurrentUser = $derived(get(currentUser)?.login === user.login);

	// README editing state
	let editingReadme = $state(false);
	let readmeContent = $state('');
	let isSavingReadme = $state(false);

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
	async function saveReadme() {
		try {
			isSavingReadme = true;
			await userService.updateUser({ readme: readmeContent });

			// Update the local user data with the new README
			user = {
				...user,
				readme: readmeContent
			};

			editingReadme = false;
			toasts.success('README updated successfully');
		} catch (error) {
			toasts.error(
				`Failed to update README: ${error instanceof Error ? error.message : 'Unknown error'}`
			);
		} finally {
			isSavingReadme = false;
		}
	}

	let patchStacksStore = $state(userService.getPatchStacks(ownerSlug));
	let patchStacks = $derived($patchStacksStore);
	let patchStacksData = $derived(patchStacks.status === 'found' ? patchStacks.value || [] : []);

	// State for projects
	let projectsLoading = $state(true);
	let projectsData = $state<any[]>([]);

	$effect(() => {
		if (ownerSlug) {
			patchStacksStore = userService.getPatchStacks(ownerSlug);
		}
	});

	// Load the user's recent projects
	$effect(() => {
		if (user?.login) {
			projectsLoading = true;
			userService
				.recentProjects(user.login)
				.then((projects) => {
					projectsData = projects;
					projectsLoading = false;
				})
				.catch((error: Error) => {
					console.error('Failed to load recent projects:', error);
					projectsLoading = false;
				});
		}
	});

	function hasContactInfo(user: ExtendedUser) {
		return user.email || user.website || user.twitter || user.bluesky || user.location;
	}

	// Use real data if available, otherwise use the fetched projects data
	let projects = $derived(projectsData);
	// Only show actual patch stacks, don't fall back to mock data
	let reviews = $derived(patchStacksData);
	let readme = $derived(user?.readme);
</script>

<div class="user-profile-page">
	<div class="content-columns">
		<div class="main-column">
			<!-- README Section -->
			<div class="section-card readme-section">
				<div class="readme-header">
					<h2 class="section-title-only">README</h2>
					{#if isCurrentUser}
						<div class="readme-actions">
							{#if editingReadme}
								<AsyncButton style="primary" action={saveReadme} disabled={isSavingReadme}>
									Save
								</AsyncButton>
								<Button
									type="button"
									style="secondary"
									onclick={cancelEditingReadme}
									disabled={isSavingReadme}
								>
									Cancel
								</Button>
							{:else}
								<Button
									type="button"
									style="secondary"
									onclick={() => startEditingReadme(user.readme)}
								>
									Edit README
								</Button>
							{/if}
						</div>
					{/if}
				</div>
				<div class="readme-content">
					{#if editingReadme}
						<textarea
							bind:value={readmeContent}
							class="readme-editor"
							rows="15"
							placeholder="Enter markdown content for your README..."
							disabled={isSavingReadme}
						></textarea>
						<div class="readme-preview">
							<h3 class="preview-title">Preview</h3>
							<Markdown content={readmeContent} />
						</div>
					{:else if readme}
						<Markdown content={readme} />
					{:else}
						<div class="no-readme">
							{#if isCurrentUser}
								<p>No README available for your profile. Click "Edit README" to create one.</p>
							{:else}
								<p>No README available for this profile.</p>
							{/if}
						</div>
					{/if}
				</div>
			</div>

			<!-- Projects Section -->
			<ProjectsSection {projects} {ownerSlug} loading={projectsLoading} />

			<!-- Reviews Section -->
			<ReviewsSection {reviews} status={patchStacks.status} />
		</div>

		<div class="side-column">
			<!-- User Profile Card - New section with avatar and name -->
			<div class="section-card profile-card">
				{#if user.avatarUrl}
					<img src={user.avatarUrl} alt="{user.name}'s avatar" class="sidebar-avatar" />
				{/if}
				<h2 class="sidebar-name">{user.name}</h2>
				<p class="sidebar-username">@{user.login}</p>
			</div>

			<!-- Contact & Info Section -->
			{#if hasContactInfo(user)}
				<div class="section-card contact-info-section">
					<h2 class="section-title">Contact & Info</h2>
					<div class="contact-info-list">
						{#if user.email}
							<div class="info-item">
								<span class="info-icon">✉️</span>
								<span class="value"><a href={`mailto:${user.email}`}>{user.email}</a></span>
							</div>
						{/if}
						{#if user.website}
							<div class="info-item">
								<span class="info-icon">🌐</span>
								<a href={user.website} target="_blank" rel="noopener noreferrer" class="info-value">
									{user.website.replace(/^https?:\/\//, '')}
								</a>
							</div>
						{/if}

						{#if user.twitter}
							<div class="info-item">
								<span class="info-icon">𝕏</span>
								<a
									href={`https://twitter.com/${user.twitter}`}
									target="_blank"
									rel="noopener noreferrer"
									class="info-value"
								>
									@{user.twitter}
								</a>
							</div>
						{/if}

						{#if user.bluesky}
							<div class="info-item">
								<span class="info-icon">🦋</span>
								<a
									href={`https://bsky.app/profile/${user.bluesky}`}
									target="_blank"
									rel="noopener noreferrer"
									class="info-value"
								>
									{user.bluesky}
								</a>
							</div>
						{/if}

						{#if user.location}
							<div class="info-item">
								<span class="info-icon">📍</span>
								<span class="info-value">{user.location}</span>
							</div>
						{/if}
					</div>
				</div>
			{/if}

			<!-- Organizations Section -->
			{#if user?.organizations && user.organizations.length > 0}
				<div class="section-card organizations-section">
					<h2 class="section-title">Organizations</h2>
					<div class="organizations-list">
						{#each user?.organizations as org}
							<div class="org-card">
								<a href="/{org.slug}" class="org-link">
									<div class="org-info">
										<span class="org-name">{org.name}</span>
										<span class="org-role">{org.description}</span>
									</div>
								</a>
							</div>
						{/each}
					</div>
				</div>
			{/if}
		</div>
	</div>
</div>

<style>
	.user-profile-page {
		color: #333;
	}

	.content-columns {
		display: grid;
		grid-template-columns: 3fr 1fr;
		gap: 2rem;
	}

	.section-card {
		background-color: white;
		border-radius: 8px;
		margin-bottom: 2rem;
		overflow: hidden;
		border: 1px solid color(srgb 0.831373 0.815686 0.807843);
	}

	.section-title {
		font-size: 0.8em;
		margin: 0;
		padding: 12px 15px;
		border-bottom: 1px solid color(srgb 0.831373 0.815686 0.807843);
		background-color: #f3f3f2;
		color: color(srgb 0.52549 0.494118 0.47451);
	}

	.section-title-only {
		font-size: 0.8em;
		margin: 0;
		padding: 12px 15px;
		color: color(srgb 0.52549 0.494118 0.47451);
	}

	.readme-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		background-color: #f3f3f2;
		border-bottom: 1px solid color(srgb 0.831373 0.815686 0.807843);
	}

	.readme-actions {
		display: flex;
		gap: 0.5rem;
		padding-right: 15px;
	}

	/* README Section */
	.readme-content {
		padding: 1.5rem;
		line-height: 1.6;
	}

	.readme-editor {
		width: 100%;
		min-height: 200px;
		padding: 0.75rem;
		margin-bottom: 1rem;
		border: 1px solid var(--border-color, #eaeaea);
		border-radius: 4px;
		font-family: var(--fontfamily-mono);
		resize: vertical;
	}

	.readme-preview {
		border-top: 1px solid var(--border-color, #eaeaea);
		padding-top: 1rem;
		margin-top: 1rem;
	}

	.preview-title {
		font-size: 1rem;
		margin: 0 0 0.75rem 0;
		color: var(--text-muted, #666);
	}

	.no-readme {
		color: #718096;
		padding: 0.5rem 0;
		text-align: center;
	}

	.readme-content :global(h1),
	.readme-content :global(h2),
	.readme-content :global(h3) {
		margin-top: 1.5rem;
		margin-bottom: 0.75rem;
	}

	.readme-content :global(p) {
		margin-bottom: 1rem;
	}

	.readme-content :global(ul),
	.readme-content :global(ol) {
		padding-left: 1.5rem;
		margin-bottom: 1rem;
	}

	.readme-content :global(code) {
		background-color: #f1f5f9;
		padding: 0.1rem 0.3rem;
		border-radius: 3px;
		font-family: var(--fontfamily-mono);
		font-size: 0.9em;
	}

	.readme-content :global(pre) {
		background-color: #1e293b;
		color: #e2e8f0;
		padding: 1rem;
		border-radius: 6px;
		overflow-x: auto;
		margin: 1rem 0;
	}

	/* Organizations */
	.organizations-list {
		padding: 0;
	}

	.org-card {
		padding: 0.75rem;
		border-bottom: 1px solid #e2e8f0;
	}

	.org-card:last-child {
		border-bottom: none;
	}

	.org-link {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		color: inherit;
		text-decoration: none;
	}

	.org-info {
		display: flex;
		flex-direction: column;
	}

	.org-name {
		font-weight: 500;
		color: #2d3748;
	}

	.org-role {
		font-size: 0.8rem;
		color: #718096;
	}

	/* Contact Info Styles */
	.contact-info-list {
		font-size: 0.8rem;
	}

	.info-item {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.75rem;
		border-bottom: 1px solid #e2e8f0;
	}

	.info-item:last-child {
		border-bottom: none;
	}

	.info-icon {
		min-width: 1.5rem;
		text-align: center;
	}

	.info-value {
		color: #4a5568;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	a.info-value {
		color: #2563eb;
		text-decoration: none;
	}

	a.info-value:hover {
		text-decoration: underline;
	}

	/* New sidebar profile card styles */
	.profile-card {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
		padding: 1.5rem;
	}

	.sidebar-avatar {
		width: 100px;
		height: 100px;
		border-radius: 50%;
		object-fit: cover;
		box-shadow: 0 4px 8px rgba(0, 0, 0, 0.1);
		margin-bottom: 1rem;
	}

	.sidebar-name {
		font-size: 1.5rem;
		margin: 0;
		line-height: 1.2;
		color: #1a202c;
	}

	.sidebar-username {
		font-size: 1rem;
		color: #718096;
		margin: 0.5rem 0 0 0;
	}

	@media (max-width: 768px) {
		.content-columns {
			grid-template-columns: 1fr;
		}
	}
</style>
