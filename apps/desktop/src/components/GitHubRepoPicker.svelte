<script lang="ts">
	import { GitHubRepoListService } from '$lib/forge/github/githubRepoListService.svelte';
	import { UserService } from '$lib/user/userService';
	import { ProjectsService } from '$lib/project/projectsService';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import ScrollableContainer from '@gitbutler/ui/scroll/ScrollableContainer.svelte';
	import { goto } from '$app/navigation';
	import newProjectSvg from '$lib/assets/illustrations/new-project.svg?raw';
	import directionDoubtSvg from '$lib/assets/illustrations/direction-doubt.svg?raw';
	import type { GitHubRepository } from '$lib/forge/github/githubRepoListService.svelte';
	import type { GitHubApi } from '$lib/state/clientState.svelte';
	import type { Project } from '$lib/project/project';

	interface Props {
		onRepoSelected: (repoUrl: string) => void;
		gitHubApi: GitHubApi;
		onClose?: () => void;
	}

	const { onRepoSelected, gitHubApi, onClose }: Props = $props();

	const userService = getContext(UserService);
	const projectsService = getContext(ProjectsService);
	const user = userService.user;
	const projects = projectsService.projects;

	let modal = $state<Modal>();
	let confirmOpenProjectModal = $state<Modal>();
	let repoListService: GitHubRepoListService | undefined = $state();
	let loading = $state(false);
	let repositories = $state<GitHubRepository[]>([]);
	let searchQuery = $state('');
	let sortDirection = $state<'asc' | 'desc'>('desc');
	let selectedExistingProject = $state<Project | undefined>(undefined);
	let selectedRepository = $state<GitHubRepository | undefined>(undefined);

	function isRepoInGitButler(repo: GitHubRepository): Project | undefined {
		const projectList = $projects;
		if (!projectList || projectList.length === 0) {
			return undefined;
		}

		const matchingProject = projectList.find((project) => {
			if (project.title === repo.name) {
				return true;
			}

			if (project.title === repo.full_name) {
				return true;
			}

			if (project.title.toLowerCase().includes(repo.name.toLowerCase())) {
				return true;
			}

			if (
				project.path &&
				repo.name &&
				project.path.toLowerCase().includes(repo.name.toLowerCase())
			) {
				return true;
			}

			const repoUrls = [repo.clone_url, repo.git_url, repo.ssh_url, repo.html_url].filter(Boolean);

			const urlMatch = repoUrls.some((url) => {
				if (!url) return false;

				const urlMatch = url.match(/github\.com[\/:]([^\/]+)\/([^\/\.]+)/);
				if (urlMatch) {
					const [, owner, repoName] = urlMatch;
					if (!owner || !repoName) return false;

					const repoIdentifier = `${owner}/${repoName}`;

					const titleMatchesFullName = project.title === repoIdentifier;
					const titleMatchesRepoName = project.title === repoName;
					const titleContainsRepoName = project.title
						.toLowerCase()
						.includes(repoName.toLowerCase());
					const pathContainsRepoName =
						project.path && project.path.toLowerCase().includes(repoName.toLowerCase());
					const pathContainsOwner =
						project.path && project.path.toLowerCase().includes(owner.toLowerCase());

					if (
						titleMatchesFullName ||
						titleMatchesRepoName ||
						titleContainsRepoName ||
						pathContainsRepoName ||
						pathContainsOwner
					) {
						return true;
					}
				}

				return false;
			});

			return urlMatch;
		});

		return matchingProject;
	}

	const filteredAndSortedRepos = $derived(
		repositories
			.filter(
				(repo) =>
					repo.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
					repo.full_name.toLowerCase().includes(searchQuery.toLowerCase()) ||
					(repo.description && repo.description.toLowerCase().includes(searchQuery.toLowerCase()))
			)
			.sort((a, b) => {
				const dateA = new Date((a.updated_at || a.created_at) ?? new Date()).getTime();
				const dateB = new Date((b.updated_at || b.created_at) ?? new Date()).getTime();
				return sortDirection === 'desc' ? dateB - dateA : dateA - dateB;
			})
	);

	const isAuthenticated = $derived($user?.github_access_token);

	export async function openModal() {
		if (!isAuthenticated) return;

		modal?.show();

		if (!repoListService) {
			repoListService = new GitHubRepoListService(gitHubApi);
		}

		try {
			await projectsService.reload();
		} catch (error) {
			console.warn('Failed to reload projects:', error);
		}

		loading = true;
		try {
			const result = await repoListService.fetchUserRepos({
				per_page: 100,
				sort: 'updated',
				type: 'owner'
			});
			if (result.data) {
				repositories = result.data;
			}
		} catch (error) {
			console.error('Failed to fetch repositories:', error);
		} finally {
			loading = false;
		}
	}

	function selectRepository(repo: GitHubRepository) {
		const existingProject = isRepoInGitButler(repo);
		if (existingProject) {
			selectedExistingProject = existingProject;
			selectedRepository = repo;
			modal?.close();
			confirmOpenProjectModal?.show();
		} else {
			onRepoSelected(repo.clone_url);
			modal?.close();
			onClose?.();
		}
	}
</script>

{#if isAuthenticated}
	<Modal bind:this={modal} width="large" title="Select Repository">
		<div class="repo-selector-split">
			<div class="repo-selector">
				<div class="repo-selector__search">
					<div class="search-container">
						<Textbox placeholder="Search repositories..." bind:value={searchQuery} icon="search" />
						<Button
							kind="outline"
							size="button"
							icon={sortDirection === 'desc' ? 'chevron-down' : 'chevron-up'}
							onclick={() => (sortDirection = sortDirection === 'desc' ? 'asc' : 'desc')}
							tooltip={`Sort by date ${sortDirection === 'desc' ? 'descending' : 'ascending'}`}
						>
							{sortDirection === 'desc' ? 'Newest' : 'Oldest'}
						</Button>
					</div>
				</div>

				<div class="repo-selector__content">
					{#if loading}
						<div class="repo-selector__state">
							<Icon name="spinner" />
							<span>Loading repositories...</span>
						</div>
					{:else if filteredAndSortedRepos.length === 0}
						<div class="repo-selector__state">
							<Icon name="folder" />
							<span>No repositories found</span>
						</div>
					{:else}
						<ScrollableContainer maxHeight="100%" whenToShow="scroll">
							<div class="repo-selector__list">
								{#each filteredAndSortedRepos as repo (repo.id)}
									<button type="button" class="repo-item" onclick={() => selectRepository(repo)}>
										<div class="repo-item__header">
											<div class="repo-item__title-row">
												<span class="repo-item__name">{repo.full_name}</span>
												<div class="repo-item__badges">
													{#if repo.private}
														<Badge style="neutral" size="icon" icon="eye-hidden" />
													{/if}
													{#if repo.language}
														<Badge style="neutral" size="icon">
															{repo.language}
														</Badge>
													{/if}
													{#if isRepoInGitButler(repo)}
														<Badge style="success" size="icon">Added</Badge>
													{/if}
												</div>
											</div>
										</div>
										{#if repo.description}
											<p class="repo-item__description">{repo.description}</p>
										{/if}
										<div class="repo-item__meta">
											<span class="repo-item__updated">
												Updated {repo.updated_at
													? new Date(repo.updated_at).toLocaleDateString()
													: 'Unknown'}
											</span>
										</div>
									</button>
								{/each}
							</div>
						</ScrollableContainer>
					{/if}
				</div>
			</div>

			<div class="repo-selector__illustration">
				<div class="repo-selector__illustration-wrapper">
					<div class="repo-selector__illustration-svg">
						{@html directionDoubtSvg}
					</div>
				</div>
				<div class="repo-selector__illustration-text text-16 text-bold">
					Select a repository to add to GitButler
				</div>
			</div>
		</div>
	</Modal>

	<!-- Confirmation modal for opening existing project -->
	<Modal
		bind:this={confirmOpenProjectModal}
		width={434}
		noPadding
		onSubmit={async (close) => {
			if (selectedExistingProject) {
				goto(`/${selectedExistingProject.id}/board`);
				close();
				onClose?.();
			}
		}}
	>
		{#if selectedExistingProject && selectedRepository}
			<div class="open-project-modal-wrapper">
				<div class="open-project-modal-illustration-wrapper">
					<div class="open-project-modal-illustration__svg">
						{@html newProjectSvg}
					</div>
				</div>
				<div class="open-project-modal-content">
					<div class="open-project-modal-description">
						<h2 class="text-16 text-bold">Great minds think alike! 🎉</h2>
						<p class="text-13 text-body">
							This repository is already added to GitButler as a local project.
						</p>

						<p class="text-13 text-body">Would you like to open this project now?</p>
					</div>
				</div>
			</div>
		{/if}
		{#snippet controls(close: () => void)}
			<div class="open-project-modal-footer">
				<div class="open-project-modal-footer__spacer"></div>
				<Button
					kind="outline"
					onclick={() => {
						close();
						modal?.show();
					}}>Cancel</Button
				>
				<Button style="pop" type="submit">Open Project</Button>
			</div>
		{/snippet}
	</Modal>
{/if}

<style lang="postcss">
	.repo-selector-split {
		display: flex;
		min-height: 450px;
		gap: 16px;
	}

	.repo-selector {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-width: 0;
		min-height: 300px;
		max-height: 450px;
		gap: 16px;
	}

	.repo-selector__illustration {
		display: none;
		flex: 0 0 280px;
		flex-direction: column;
		margin-left: auto;
		border-radius: var(--radius-m);
		background-color: var(--clr-illustration-bg);
	}

	.repo-selector__illustration-wrapper {
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: center;
		padding: 16px;
	}

	.repo-selector__illustration-svg {
		width: 100%;
		max-width: 320px;
		height: auto;
	}

	.repo-selector__illustration-svg :global(svg) {
		width: 100%;
		height: auto;
	}

	.repo-selector__illustration-text {
		max-width: none;
		padding: 20px 16px 40px;
		color: var(--clr-scale-pop-30);
		line-height: 1.4;
		text-align: center;
	}

	@media (min-width: 900px) {
		.repo-selector__illustration {
			display: flex;
		}

		.repo-selector-split {
			min-height: 500px;
		}

		.repo-selector {
			max-height: 500px;
		}
	}

	.repo-selector__search {
		flex-shrink: 0;
	}

	.search-container {
		display: flex;
		align-items: center;
		width: 100%;
		gap: 12px;
	}

	.search-container :global(.textbox) {
		flex: 0 0 60%;
		width: 60%;
	}

	.search-container :global(.btn) {
		flex: 0 0 auto;
	}

	.repo-selector__content {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-height: 0;
	}

	.repo-selector__state {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 40px 20px;
		gap: 12px;
		color: var(--clr-text-2);
		text-align: center;
	}

	.repo-selector__list {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.repo-item {
		display: flex;
		position: relative;
		flex-direction: column;
		padding: 12px 14px;
		gap: 8px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background: var(--clr-bg-1);
		text-align: left;
		cursor: pointer;
		transition: all 0.15s ease;

		&::after {
			position: absolute;
			top: 10px;
			left: 0;
			width: 5px;
			height: calc(100% - 20px);
			transform: translateX(-100%);
			border-radius: 0 var(--radius-m) var(--radius-m) 0;
			background-color: var(--clr-selected-in-focus-element);
			content: '';
			transition: transform var(--transition-medium);
		}

		&:hover {
			border-color: var(--clr-border-3);
			background: var(--clr-bg-1-muted);

			&::after {
				transform: translateX(0);
			}
		}

		&:active {
			transform: scale(0.98);
		}

		&:not(:last-child) {
			border-bottom: 1px solid var(--clr-border-2);
		}
	}

	.repo-item__header {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.repo-item__title-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
	}

	.repo-item__name {
		flex: 1;
		overflow: hidden;
		color: var(--clr-text-1);
		font-weight: 600;
		font-size: 14px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.repo-item__badges {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		gap: 4px;
	}

	.repo-item__description {
		display: -webkit-box;
		margin: 0;
		overflow: hidden;
		color: var(--clr-text-2);
		font-size: 13px;
		line-height: 1.4;
		-webkit-line-clamp: 2;
		-webkit-box-orient: vertical;
	}

	.repo-item__meta {
		display: flex;
		align-items: center;
		color: var(--clr-text-3);
		font-size: 12px;
	}

	.repo-item__updated {
	}

	.open-project-modal-wrapper {
		display: flex;
		flex-direction: column;
	}

	.open-project-modal-illustration-wrapper {
		position: relative;
		height: 176px;
		background-color: var(--clr-illustration-bg);
	}

	.open-project-modal-illustration__svg {
		position: absolute;
		bottom: 0;
		left: 16px;
		width: 404px;
		height: 158px;
	}

	.open-project-modal-content {
		display: flex;
		flex-direction: column;
		padding: 20px 16px 16px;
		gap: 10px;
	}

	.open-project-modal-description {
		display: flex;
		flex-direction: column;
		max-width: 380px;
		gap: 10px;
	}

	.open-project-modal-footer {
		display: flex;
		width: 100%;
		gap: 6px;
	}

	.open-project-modal-footer__spacer {
		flex: 1;
	}
</style>
