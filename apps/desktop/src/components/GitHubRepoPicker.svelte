<script lang="ts">
	import { GitHubRepoListService } from '$lib/forge/github/githubRepoListService.svelte';
	import { UserService } from '$lib/user/userService';
	import { ProjectsService } from '$lib/project/projectsService';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import ScrollableContainer from '@gitbutler/ui/scroll/ScrollableContainer.svelte';
	import { getFileIcon } from '@gitbutler/ui/file/getFileIcon';
	import { goto } from '$app/navigation';
	import newProjectSvg from '$lib/assets/illustrations/new-project.svg?raw';
	import type { GitHubRepository } from '$lib/forge/github/githubRepoListService.svelte';
	import type { GitHubApi } from '$lib/state/clientState.svelte';
	import type { Project } from '$lib/project/project';

	interface Props {
		onRepoSelected: (repoUrl: string) => void;
		gitHubApi: GitHubApi;
		onClose?: () => void;
	}

	const { onRepoSelected, gitHubApi, onClose }: Props = $props();

	// Map GitHub language names to file extensions for icon lookup
	function getLanguageIcon(language: string): string {
		const languageMap: Record<string, string> = {
			JavaScript: 'js',
			TypeScript: 'ts',
			Python: 'py',
			Rust: 'rs',
			Java: 'java',
			'C++': 'cpp',
			'C#': 'cs',
			PHP: 'php',
			Ruby: 'rb',
			Go: 'go',
			Swift: 'swift',
			Kotlin: 'kt',
			Dart: 'dart',
			Scala: 'scala',
			HTML: 'html',
			CSS: 'css',
			Vue: 'vue',
			Svelte: 'svelte',
			Shell: 'sh',
			PowerShell: 'ps1',
			Dockerfile: 'dockerfile',
			YAML: 'yaml',
			JSON: 'json',
			XML: 'xml',
			Markdown: 'md',
			Lua: 'lua',
			Perl: 'perl',
			R: 'r',
			Julia: 'jl',
			Haskell: 'hs',
			Clojure: 'clj',
			Elixir: 'ex',
			Erlang: 'erl',
			'F#': 'fs',
			OCaml: 'ml',
			Nim: 'nim',
			Crystal: 'cr',
			Zig: 'zig',
			C: 'c'
		};

		const extension = languageMap[language] || 'txt';
		return getFileIcon(`dummy.${extension}`);
	}

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
	let sortDirection = $state<'asc' | 'desc'>('desc'); // Default to newest first
	let selectedExistingProject = $state<Project | undefined>(undefined);
	let selectedRepository = $state<GitHubRepository | undefined>(undefined);

	// Debug reactive statement to log when projects change
	$effect(() => {
		console.log('Projects changed:', $projects?.length || 0, 'projects available');
		if ($projects && $projects.length > 0) {
			console.log(
				'First few projects:',
				$projects.slice(0, 3).map((p) => ({ title: p.title, path: p.path }))
			);
		}
	});

	// Function to check if a repository is already available in GitButler
	function isRepoInGitButler(repo: GitHubRepository): Project | undefined {
		const projectList = $projects;
		if (!projectList || projectList.length === 0) {
			console.log('No projects available for matching');
			return undefined;
		}

		console.log(`Checking repo: ${repo.full_name}`);
		console.log(
			'Available projects:',
			projectList.map((p) => ({
				title: p.title,
				path: p.path,
				id: p.id
			}))
		);

		const matchingProject = projectList.find((project) => {
			// 1. Check by exact repository name match
			if (project.title === repo.name) {
				console.log(`✓ Found exact name match: ${project.title} === ${repo.name}`);
				return true;
			}

			// 2. Check by full name match (owner/repo)
			if (project.title === repo.full_name) {
				console.log(`✓ Found full name match: ${project.title} === ${repo.full_name}`);
				return true;
			}

			// 3. Check if project title contains the repo name
			if (project.title.toLowerCase().includes(repo.name.toLowerCase())) {
				console.log(`✓ Found title contains repo name: ${project.title} contains ${repo.name}`);
				return true;
			}

			// 4. Check if the project path contains the repository name
			if (
				project.path &&
				repo.name &&
				project.path.toLowerCase().includes(repo.name.toLowerCase())
			) {
				console.log(`✓ Found path contains repo name: ${project.path} contains ${repo.name}`);
				return true;
			}

			// 5. Check by extracting repo name from common GitHub URLs
			const repoUrls = [repo.clone_url, repo.git_url, repo.ssh_url, repo.html_url].filter(Boolean);

			const urlMatch = repoUrls.some((url) => {
				if (!url) return false;

				// Extract the repo identifier from the URL (owner/repo)
				const urlMatch = url.match(/github\.com[\/:]([^\/]+)\/([^\/\.]+)/);
				if (urlMatch) {
					const [, owner, repoName] = urlMatch;
					if (!owner || !repoName) return false;

					const repoIdentifier = `${owner}/${repoName}`;

					// Check various combinations
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
						console.log(`✓ Found URL-based match: ${url} -> ${repoIdentifier}`);
						console.log(`  - titleMatchesFullName: ${titleMatchesFullName}`);
						console.log(`  - titleMatchesRepoName: ${titleMatchesRepoName}`);
						console.log(`  - titleContainsRepoName: ${titleContainsRepoName}`);
						console.log(`  - pathContainsRepoName: ${pathContainsRepoName}`);
						console.log(`  - pathContainsOwner: ${pathContainsOwner}`);
						return true;
					}
				}

				return false;
			});

			return urlMatch;
		});

		console.log(`Repo ${repo.full_name} is ${matchingProject ? 'IN' : 'NOT IN'} GitButler`);
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

		// Ensure we have the latest projects
		try {
			await projectsService.reload();
			console.log('Projects reloaded, current count:', $projects?.length || 0);
		} catch (error) {
			console.warn('Failed to reload projects:', error);
		}

		// Load repositories
		loading = true;
		try {
			const result = await repoListService.fetchUserRepos({
				per_page: 100,
				sort: 'updated',
				type: 'owner'
			});
			if (result.data) {
				repositories = result.data;
				console.log('Loaded repositories:', repositories.length);
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
			// Close the main modal and open confirmation modal
			selectedExistingProject = existingProject;
			selectedRepository = repo;
			modal?.close();
			confirmOpenProjectModal?.show();
		} else {
			// Proceed with importing the repository
			onRepoSelected(repo.clone_url);
			modal?.close();
			onClose?.();
		}
	}
</script>

{#if isAuthenticated}
	<Modal bind:this={modal} width="medium" title="Select Repository">
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
					<ScrollableContainer maxHeight="400px" whenToShow="scroll">
						<div class="repo-selector__list">
							{#each filteredAndSortedRepos as repo (repo.id)}
								<button type="button" class="repo-item" onclick={() => selectRepository(repo)}>
									<div class="repo-item__header">
										<span class="repo-item__name">{repo.full_name}</span>
										{#if repo.private}
											<div class="repo-item__private-badge">
												<Icon name="eye-hidden" size={12} />
											</div>
										{/if}
									</div>
									{#if repo.description}
										<p class="repo-item__description">{repo.description}</p>
									{/if}
									<div class="repo-item__meta">
										{#if repo.language}
											<div class="repo-item__language-badge">
												<img
													src={getLanguageIcon(repo.language)}
													alt={repo.language}
													width="12"
													height="12"
												/>
												<span>{repo.language}</span>
											</div>
										{/if}
										{#if isRepoInGitButler(repo)}
											<div class="repo-item__gitbutler-badge">
												<Icon name="git" size={12} />
												<span>In GitButler</span>
											</div>
										{/if}
										<span class="repo-item__divider">•</span>
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
						<h2 class="text-16 text-bold">Project Already in GitButler</h2>
						<p class="text-13 text-body">
							This repository is already added to GitButler as a local project.
						</p>
						
						<div class="open-project-modal-badges">
							<div class="repo-badge github-badge">
								<Icon name="github" size={12} />
								<span>{selectedRepository.name}</span>
							</div>
							<div class="badge-arrow">
								<Icon name="arrow-right" size={14} />
							</div>
							<div class="repo-badge local-badge">
								<Icon name="folder" size={12} />
								<span>{selectedExistingProject.title}</span>
							</div>
						</div>
						
						<p class="text-13 text-body">
							Would you like to open this project now?
						</p>
					</div>
				</div>
			</div>
		{/if}
		{#snippet controls(close: () => void)}
			<div class="open-project-modal-footer">
				<div class="open-project-modal-footer__spacer"></div>
				<Button kind="outline" onclick={close}>Cancel</Button>
				<Button style="pop" type="submit">Open Project</Button>
			</div>
		{/snippet}
	</Modal>
{/if}

<style lang="postcss">
	.repo-selector {
		display: flex;
		flex-direction: column;
		min-height: 300px;
		max-height: 500px;
		gap: 16px;
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
		flex: 1;
		min-height: 0;
	}

	.repo-selector__state {
		display: flex;
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
		padding: 14px;
		gap: 10px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background: var(--clr-bg-1);
		text-align: left;
		cursor: pointer;
		transition: all 0.15s ease;

		&::after {
			position: absolute;
			top: 12px;
			left: 0;
			width: 5px;
			height: calc(100% - 24px);
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

	.repo-item__private-badge {
		display: flex;
		align-items: center;
		padding: 2px 6px;
		border-radius: var(--radius-s);
		background-color: var(--clr-bg-2);
		color: var(--clr-text-3);
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
		gap: 8px;
		color: var(--clr-text-3);
		font-size: 12px;
	}

	.repo-item__language-badge {
		display: flex;
		align-items: center;
		padding: 2px 6px;
		gap: 3px;
		border-radius: 8px;
		background-color: var(--clr-theme-pop-element);
		color: var(--clr-theme-pop-on-element);
		font-weight: 400;
		font-size: 10px;
	}

	.repo-item__language-badge img {
		/* Default - make icons white for better contrast on colored background */
		filter: brightness(0) invert(1);
	}

	.repo-item__gitbutler-badge {
		display: flex;
		align-items: center;
		padding: 2px 6px;
		gap: 3px;
		border-radius: 8px;
		background-color: var(--clr-theme-succ-element);
		color: var(--clr-theme-succ-on-element);
		font-weight: 400;
		font-size: 10px;
	}

	.repo-item__gitbutler-badge :global(svg) {
		color: var(--clr-theme-succ-on-element);
	}

	/* Light theme adjustments */
	:global([data-theme='light']) .repo-item__language-badge img {
		filter: brightness(0) invert(1);
	}

	/* Ensure proper contrast in all themes */
	@media (prefers-color-scheme: light) {
		.repo-item__language-badge img {
			filter: brightness(0) invert(1);
		}
	}

	@media (prefers-color-scheme: dark) {
		.repo-item__language-badge img {
			filter: brightness(0) invert(1);
		}
	}

	.repo-item__divider {
		color: var(--clr-text-3);
		opacity: 0.5;
	}

	.repo-item__updated {
		margin-left: auto;
	}

	/* Open project modal styles - matching TryV3Modal */
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

	.open-project-modal-badges {
		display: flex;
		align-items: center;
		justify-content: center;
		margin: 16px 0;
		gap: 8px;
		flex-wrap: wrap;
	}

	.repo-badge {
		display: inline-flex;
		align-items: center;
		padding: 4px 8px;
		gap: 4px;
		border-radius: 12px;
		font-weight: 500;
		font-size: 11px;
		white-space: nowrap;
		min-width: fit-content;
		max-width: 120px;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.github-badge {
		background-color: var(--clr-theme-pop-element);
		color: var(--clr-theme-pop-on-element);
		border: 1px solid var(--clr-theme-pop-element);
	}

	.local-badge {
		background-color: var(--clr-theme-succ-element);
		color: var(--clr-theme-succ-on-element);
		border: 1px solid var(--clr-theme-succ-element);
	}

	.badge-arrow {
		display: flex;
		align-items: center;
		color: var(--clr-text-3);
		opacity: 0.6;
		flex-shrink: 0;
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
