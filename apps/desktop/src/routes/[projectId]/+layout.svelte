<script lang="ts">
	import { goto } from '$app/navigation';
	import AnalyticsMonitor from '$components/AnalyticsMonitor.svelte';
	import Chrome from '$components/Chrome.svelte';
	import FileMenuAction from '$components/FileMenuAction.svelte';
	import FullviewLoading from '$components/FullviewLoading.svelte';
	import IrcPopups from '$components/IrcPopups.svelte';
	import NoBaseBranch from '$components/NoBaseBranch.svelte';
	import NotOnGitButlerBranch from '$components/NotOnGitButlerBranch.svelte';
	import ProblemLoadingRepo from '$components/ProblemLoadingRepo.svelte';
	import ProjectSettingsMenuAction from '$components/ProjectSettingsMenuAction.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { OnboardingEvent, POSTHOG_WRAPPER } from '$lib/analytics/posthog';
	import { BACKEND } from '$lib/backend';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { BRANCH_SERVICE } from '$lib/branches/branchService.svelte';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { GITHUB_CLIENT } from '$lib/forge/github/githubClient';
	import { useGitHubAccessToken } from '$lib/forge/github/hooks.svelte';
	import { GITLAB_CLIENT } from '$lib/forge/gitlab/gitlabClient.svelte';
	import { GITLAB_STATE } from '$lib/forge/gitlab/gitlabState.svelte';
	import { GIT_SERVICE } from '$lib/git/gitService';
	import { MODE_SERVICE } from '$lib/mode/modeService';
	import { showError, showInfo, showWarning } from '$lib/notifications/toasts';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { FILE_SELECTION_MANAGER } from '$lib/selection/fileSelectionManager.svelte';
	import { UNCOMMITTED_SERVICE } from '$lib/selection/uncommittedService.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { CLIENT_STATE } from '$lib/state/clientState.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { debounce } from '$lib/utils/debounce';
	import { WORKTREE_SERVICE } from '$lib/worktree/worktreeService.svelte';
	import { inject } from '@gitbutler/core/context';
	import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
	import { onDestroy, untrack, type Snippet } from 'svelte';
	import type { LayoutData } from './$types';

	const { data, children: pageChildren }: { data: LayoutData; children: Snippet } = $props();

	// =============================================================================
	// PROJECT SETUP & CORE STATE
	// =============================================================================

	const { projectId } = $derived(data);

	// Core services
	const posthog = inject(POSTHOG_WRAPPER);
	const settingsService = inject(SETTINGS_SERVICE);
	const settingsStore = settingsService.appSettings;
	const projectsService = inject(PROJECTS_SERVICE);
	const clientState = inject(CLIENT_STATE);

	// Project data
	const projectsQuery = $derived(projectsService.projects());
	const projects = $derived(projectsQuery.response);
	const currentProject = $derived(projects?.find((p) => p.id === projectId));

	// =============================================================================
	// REPOSITORY & BRANCH MANAGEMENT
	// =============================================================================

	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const branchService = inject(BRANCH_SERVICE);
	const gitService = inject(GIT_SERVICE);

	const repoInfoQuery = $derived(baseBranchService.repo(projectId));
	const pushRepoQuery = $derived(baseBranchService.pushRepo(projectId));

	const repoInfo = $derived(repoInfoQuery.response);
	const forkInfo = $derived(pushRepoQuery.response);

	const baseBranchQuery = $derived(baseBranchService.baseBranch(projectId));
	const baseBranch = $derived(baseBranchQuery.response);
	const baseBranchName = $derived(baseBranch?.shortName);

	// =============================================================================
	// WORKSPACE & MODE MANAGEMENT
	// =============================================================================

	const modeService = inject(MODE_SERVICE);
	const stackService = inject(STACK_SERVICE);
	const worktreeService = inject(WORKTREE_SERVICE);

	const modeQuery = $derived(modeService.mode({ projectId }));
	const mode = $derived(modeQuery.response);

	// Invalidate stacks when switching branches outside workspace
	$effect(() => {
		if (mode?.type === 'OutsideWorkspace' && mode.subject.branchName) {
			stackService.invalidateStacks();
		}
	});

	// =============================================================================
	// FORGE INTEGRATION (GitHub & GitLab)
	// =============================================================================

	const gitHubClient = inject(GITHUB_CLIENT);
	const gitLabState = inject(GITLAB_STATE);
	const gitLabClient = inject(GITLAB_CLIENT);
	const forgeFactory = inject(DEFAULT_FORGE_FACTORY);

	const githubAccessToken = useGitHubAccessToken(reactive(() => projectId));

	// GitHub setup
	$effect.pre(() => gitHubClient.setToken(githubAccessToken.accessToken.current));
	$effect.pre(() => gitHubClient.setHost(githubAccessToken.host.current));
	$effect.pre(() => gitHubClient.setRepo({ owner: repoInfo?.owner, repo: repoInfo?.name }));

	// GitLab setup
	const gitlabConfigured = $derived(gitLabState.configured);
	$effect.pre(() => {
		gitLabState.init(projectId, repoInfo);
		gitLabClient.set(); // Temporary fix, will refactor.
	});

	// Forge factory configuration
	$effect(() => {
		forgeFactory.setConfig({
			repo: repoInfo,
			pushRepo: forkInfo,
			baseBranch: baseBranchName,
			githubAuthenticated: !!githubAccessToken.accessToken.current,
			githubIsLoading: githubAccessToken.isLoading.current,
			githubError: githubAccessToken.error.current,
			gitlabAuthenticated: !!$gitlabConfigured,
			detectedForgeProvider: baseBranch?.forgeRepoInfo?.forge ?? undefined,
			forgeOverride: projects?.find((project) => project.id === projectId)?.forge_override
		});
	});

	// =============================================================================
	// FILE SELECTION & WORKTREE MANAGEMENT
	// ================================================ReorderDropzoneFactory

	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const idSelection = inject(FILE_SELECTION_MANAGER);

	const worktreeDataQuery = $derived(worktreeService.worktreeData(projectId));
	const worktreeData = $derived(worktreeDataQuery.response);

	// Bridge between RTKQ and custom slice
	$effect(() => {
		if (worktreeData) {
			untrack(() => {
				uncommittedService.updateData({
					changes: worktreeData.rawChanges,
					assignments: worktreeData.hunkAssignments
				});
			});
		}
	});

	// Clear expired file selections
	const affectedPaths = $derived(worktreeData?.rawChanges.map((c) => c.path));
	$effect(() => {
		if (affectedPaths) {
			untrack(() => {
				idSelection.retain(affectedPaths);
			});
		}
	});

	// =============================================================================
	// ANALYTICS INIT
	// =============================================================================

	$effect(() => {
		posthog.setPostHogRepo(repoInfo);
		return () => {
			posthog.setPostHogRepo(undefined);
		};
	});

	// =============================================================================
	// WINDOW TITLE
	// =============================================================================

	const backend = inject(BACKEND);
	$effect(() => {
		let baseTitle: string;
		let windowTitle: string;
		const projectTitle = currentProject?.title;

		Promise.all([backend.getAppInfo(), backend.getWindowTitle()]).then(
			([appInfo, currentTitle]) => {
				baseTitle = appInfo.name;

				if (!currentTitle.includes(' — ')) {
					windowTitle = currentTitle;
				}

				if (projectTitle) {
					backend.setWindowTitle(`${projectTitle} — ${baseTitle}`);
				}
			}
		);

		return () => {
			if (windowTitle) {
				backend.setWindowTitle(windowTitle);
			} else if (baseTitle) {
				backend.setWindowTitle(baseTitle);
			}
		};
	});

	// =============================================================================
	// FEED & UPDATES MANAGEMENT
	// =============================================================================

	// Listen for stack details updates
	$effect(() => {
		stackService.stackDetailsUpdateListener(projectId);
	});

	// =============================================================================
	// AUTO-REFRESH & SYNCHRONIZATION
	// =============================================================================

	let intervalId: any;

	const debouncedBaseBranchRefresh = debounce(async () => {
		await baseBranchService.refreshBaseBranch(projectId).catch((error) => {
			console.error('Failed to refresh base branch:', error);
		});
	}, 500);

	const debouncedRemoteBranchRefresh = debounce(async () => {
		await branchService.refresh().catch((error) => {
			console.error('Failed to refresh remote branches:', error);
		});
	}, 500);

	// Refresh on git fetch events
	$effect(() =>
		gitService.onFetch(data.projectId, () => {
			debouncedBaseBranchRefresh();
			debouncedRemoteBranchRefresh();
		})
	);

	// Refresh when branch data changes
	$effect(() => {
		if (baseBranch || modeQuery.response) debouncedRemoteBranchRefresh();
	});

	// Auto-fetch setup
	async function fetchRemoteForProject() {
		await baseBranchService.fetchFromRemotes(projectId, 'auto');
	}

	function setupFetchInterval() {
		const autoFetchIntervalMinutes = $settingsStore?.fetch.autoFetchIntervalMinutes || 15;
		clearFetchInterval();

		if (autoFetchIntervalMinutes < 0) {
			return;
		}
		fetchRemoteForProject();
		const intervalMs = autoFetchIntervalMinutes * 60 * 1000;
		intervalId = setInterval(async () => await fetchRemoteForProject(), intervalMs);

		return () => clearFetchInterval();
	}

	function clearFetchInterval() {
		if (intervalId) clearInterval(intervalId);
	}

	// =============================================================================
	// PROJECT LIFECYCLE & NAVIGATION
	// =============================================================================

	// Setup auto-fetch when project changes
	$effect(() => {
		if (projectId) {
			untrack(() => setupFetchInterval());
		} else {
			goto('/onboarding');
		}
	});

	// Set active project and handle notifications
	async function setActiveProjectOrRedirect(projectId: string) {
		const dontShowAgainKey = `git-filters--dont-show-again--${projectId}`;
		try {
			const info = await projectsService.setActiveProject(projectId);
			posthog.captureOnboarding(OnboardingEvent.SetProjectActive);

			if (!info) return;

			if (!info.is_exclusive) {
				showInfo(
					'Just FYI, this project is already open in another window',
					'There might be some unexpected behavior if you open it in multiple windows'
				);
			}

			if (info.db_error) {
				showError('The database was corrupted', info.db_error);
			}

			if (info.headsup && localStorage.getItem(dontShowAgainKey) !== '1') {
				showWarning('Important PSA', info.headsup, {
					label: "Don't show again",
					onClick: (dismiss) => {
						localStorage.setItem(dontShowAgainKey, '1');
						dismiss();
					}
				});
			}
		} catch (error: unknown) {
			posthog.captureOnboarding(OnboardingEvent.SetProjectActiveFailed);
			showError('Failed to set the project active', error);
		}
	}

	$effect(() => {
		setActiveProjectOrRedirect(projectId);
	});

	// Clear backend API state when project changes
	$effect(() => {
		if (projectId) {
			clientState.backendApi.util.resetApiState();
		}
	});

	// Cleanup on destroy
	onDestroy(() => {
		clearFetchInterval();
	});
</script>

<ProjectSettingsMenuAction {projectId} />
<FileMenuAction />

<ReduxResult {projectId} result={combineResults(baseBranchQuery.result, modeQuery.result)}>
	{#snippet children([baseBranch, mode], { projectId })}
		{#if !baseBranch}
			<NoBaseBranch {projectId} />
		{:else if baseBranch}
			{#if mode.type === 'OpenWorkspace' || mode.type === 'Edit' || ($settingsStore?.featureFlags.singleBranch && mode.subject.branchName)}
				<div class="view-wrap" role="group" ondragover={(e) => e.preventDefault()}>
					<Chrome {projectId} sidebarDisabled={mode.type === 'Edit'}>
						{@render pageChildren()}
					</Chrome>
				</div>
			{:else if mode.type === 'OutsideWorkspace'}
				<NotOnGitButlerBranch {projectId} {baseBranch}>
					{@render pageChildren()}
				</NotOnGitButlerBranch>
			{/if}
		{/if}
	{/snippet}
	{#snippet loading()}
		<FullviewLoading />
	{/snippet}
	{#snippet error(baseError)}
		<ProblemLoadingRepo {projectId} error={baseError} />
	{/snippet}
</ReduxResult>

<!-- {#if $settingsStore?.featureFlags.v3} -->
<IrcPopups />
<!-- {/if} -->

<AnalyticsMonitor {projectId} />

<style>
	.view-wrap {
		display: flex;
		position: relative;
		width: 100%;
	}
</style>
