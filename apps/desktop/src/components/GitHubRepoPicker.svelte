<script lang="ts">
	import { GitHubRepoListService } from '$lib/forge/github/githubRepoListService.svelte';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import ScrollableContainer from '@gitbutler/ui/scroll/ScrollableContainer.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import type { GitHubApi } from '$lib/state/clientState.svelte';
	import type { GitHubRepository } from '$lib/forge/github/githubRepoListService.svelte';

	interface Props {
		onRepoSelected: (repoUrl: string) => void;
		gitHubApi: GitHubApi;
		onClose?: () => void;
	}

	const { onRepoSelected, gitHubApi, onClose }: Props = $props();

	const userService = getContext(UserService);
	const user = userService.user;

	let modal = $state<Modal>();
	let repoListService: GitHubRepoListService | undefined = $state();
	let loading = $state(false);
	let repositories = $state<GitHubRepository[]>([]);
	let searchQuery = $state('');

	const filteredRepos = $derived(
		repositories.filter(
			(repo) =>
				repo.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
				repo.full_name.toLowerCase().includes(searchQuery.toLowerCase()) ||
				(repo.description && repo.description.toLowerCase().includes(searchQuery.toLowerCase()))
		)
	);

	const isAuthenticated = $derived($user?.github_access_token);

	export async function openModal() {
		if (!isAuthenticated) return;

		modal?.show();

		if (!repoListService) {
			repoListService = new GitHubRepoListService(gitHubApi);
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
			}
		} catch (error) {
			console.error('Failed to fetch repositories:', error);
		} finally {
			loading = false;
		}
	}

	function selectRepository(repo: GitHubRepository) {
		onRepoSelected(repo.clone_url);
		modal?.close();
		onClose?.();
	}
</script>

{#if isAuthenticated}
	<Modal bind:this={modal} width="medium" title="Select Repository">
		<div class="repo-selector">
			<div class="repo-selector__search">
				<Textbox placeholder="Search repositories..." bind:value={searchQuery} icon="search" />
			</div>

			<div class="repo-selector__content">
				{#if loading}
					<div class="repo-selector__state">
						<Icon name="spinner" />
						<span>Loading repositories...</span>
					</div>
				{:else if filteredRepos.length === 0}
					<div class="repo-selector__state">
						<Icon name="folder" />
						<span>No repositories found</span>
					</div>
				{:else}
					<ScrollableContainer maxHeight="400px" whenToShow="scroll">
						<div class="repo-selector__list">
							{#each filteredRepos as repo (repo.id)}
								<button type="button" class="repo-item" onclick={() => selectRepository(repo)}>
									<div class="repo-item__header">
										<Icon name="folder" />
										<span class="repo-item__name">{repo.full_name}</span>
										{#if repo.private}
											<Icon name="eye-hidden" size={12} />
										{/if}
									</div>
									{#if repo.description}
										<p class="repo-item__description">{repo.description}</p>
									{/if}
									<div class="repo-item__meta">
										{#if repo.language}
											<span class="repo-item__language">{repo.language}</span>
										{/if}
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
		flex-direction: column;
		padding: 12px;
		gap: 8px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background: var(--clr-bg-1);
		text-align: left;
		cursor: pointer;
		transition: all 0.15s ease;

		&:hover {
			border-color: var(--clr-border-3);
			background: var(--clr-bg-1-muted);
		}

		&:active {
			transform: scale(0.98);
		}
	}

	.repo-item__header {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.repo-item__name {
		flex: 1;
		color: var(--clr-text-1);
		font-weight: 600;
	}

	.repo-item__description {
		margin: 0;
		color: var(--clr-text-2);
		font-size: 13px;
		line-height: 1.4;
	}

	.repo-item__meta {
		display: flex;
		align-items: center;
		gap: 12px;
		color: var(--clr-text-3);
		font-size: 12px;
	}

	.repo-item__language {
		font-weight: 500;
	}

	.repo-item__updated {
		margin-left: auto;
	}
</style>
