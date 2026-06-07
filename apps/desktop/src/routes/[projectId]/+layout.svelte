<script lang="ts">
	import { goto } from "$app/navigation";
	import IrcChatWindow from "$components/irc/IrcChatWindow.svelte";
	import ProjectSettingsShortcutHandler from "$components/settings/ProjectSettingsShortcutHandler.svelte";
	import AnalyticsMonitor from "$components/shared/AnalyticsMonitor.svelte";
	import FullviewLoading from "$components/shared/FullviewLoading.svelte";
	import NotOnGitButlerBranch from "$components/shared/NotOnGitButlerBranch.svelte";
	import ProjectShortcutHandler from "$components/shared/ProjectShortcutHandler.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import AppLayout from "$components/views/AppLayout.svelte";
	import NoBaseBranch from "$components/views/NoBaseBranch.svelte";
	import ProblemLoadingRepo from "$components/views/ProblemLoadingRepo.svelte";
	import { BACKEND } from "$lib/backend";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { BRANCH_SERVICE } from "$lib/branches/branchService.svelte";
	import { showError } from "$lib/error/showError";
	import { GITLAB_USER_SERVICE } from "$lib/forge/gitlab/gitlabUserService.svelte";
	import { GIT_SERVICE } from "$lib/git/gitService";
	import { IRC_API_SERVICE } from "$lib/irc/ircApiService";
	import { projectChannel } from "$lib/irc/protocol";
	import { WORKING_FILES_BROADCAST } from "$lib/irc/workingFilesBroadcast.svelte";
	import { MODE_SERVICE } from "$lib/mode/modeService";
	import { showInfo, showWarning } from "$lib/notifications/toasts";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { FILE_SELECTION_MANAGER } from "$lib/selection/fileSelectionManager.svelte";
	import { UNCOMMITTED_SERVICE } from "$lib/selection/uncommittedService.svelte";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { CLIENT_STATE } from "$lib/state/clientState.svelte";
	import { combineResults } from "$lib/state/helpers";
	import { invalidatesList, ReduxTag } from "$lib/state/tags";
	import { OnboardingEvent, POSTHOG_WRAPPER } from "$lib/telemetry/posthog";
	import { debounce } from "$lib/utils/debounce";
	import { WORKTREE_SERVICE } from "$lib/worktree/worktreeService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { mergeUnlisten } from "@gitbutler/ui/utils/mergeUnlisten";
	import { onDestroy, untrack, type Snippet } from "svelte";
	import type { LayoutData } from "./$types";

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

	const repoInfo = $derived(repoInfoQuery.response);

	const baseBranchQuery = $derived(baseBranchService.baseBranch(projectId));
	const baseBranch = $derived(baseBranchQuery.response);

	// =============================================================================
	// WORKSPACE & MODE MANAGEMENT
	// =============================================================================

	const modeService = inject(MODE_SERVICE);
	const stackService = inject(STACK_SERVICE);
	const worktreeService = inject(WORKTREE_SERVICE);

	const modeQuery = $derived(modeService.mode(projectId));

	// =============================================================================
	// FORGE INTEGRATION (GitHub & GitLab)
	// =============================================================================

	const gitlabUserService = inject(GITLAB_USER_SERVICE);

	// Migrate stored GitLab access token from the legacy location to the
	// per-account secrets entry on app load. Safe no-op when already done.
	$effect(() => {
		if (projectId) {
			gitlabUserService.migrate(projectId);
		}
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
					assignments: worktreeData.hunkAssignments,
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

				if (!currentTitle.includes(" — ")) {
					windowTitle = currentTitle;
				}

				if (projectTitle) {
					backend.setWindowTitle(`${projectTitle} — ${baseTitle}`);
				}
			},
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

	const headResponse = $derived(modeService.head(projectId));
	const head = $derived(headResponse.response);

	// Invalidate caches in response to backend events.
	$effect(() =>
		mergeUnlisten(
			backend.listen(`project://${projectId}/hunk-assignment-update`, () => {
				stackService.invalidateStacksAndDetails();
			}),
			backend.listen(`project://${projectId}/worktree_changes`, () => {
				clientState.dispatch(
					clientState.backendApi.util.invalidateTags([invalidatesList(ReduxTag.Diff)]),
				);
			}),
			backend.listen(`project://${projectId}/rule-updates`, () => {
				clientState.dispatch(
					clientState.backendApi.util.invalidateTags([invalidatesList(ReduxTag.WorkspaceRules)]),
				);
			}),
			// Activity that requires re-reading workspace state — emitted on
			// remote-ref updates (push, external fetch) and on external
			// writes to `virtual_branches.toml` (e.g. by the `but` CLI).
			// Picks up PR numbers written by external tools without a forge
			// round-trip.
			backend.listen(`project://${projectId}/workspace-activity`, () => {
				clientState.dispatch(
					clientState.backendApi.util.invalidateTags([
						invalidatesList(ReduxTag.Stacks),
						invalidatesList(ReduxTag.StackDetails),
						invalidatesList(ReduxTag.BranchListing),
					]),
				);
			}),
		),
	);

	// If the head changes, invalidate stacks and details
	// We need to track the previous head value to avoid infinite loops
	let previousHead = $state<string | undefined>(undefined);
	$effect(() => {
		if (head && head !== previousHead) {
			untrack(() => {
				previousHead = head;
				stackService.invalidateStacksAndDetails();
			});
		}
	});

	// =============================================================================
	// AUTO-REFRESH & SYNCHRONIZATION
	// =============================================================================

	let intervalId: any;

	const debouncedBaseBranchRefresh = debounce(async () => {
		await baseBranchService.refreshBaseBranch(projectId).catch((error) => {
			console.error("Failed to refresh base branch:", error);
		});
	}, 500);

	const debouncedRemoteBranchRefresh = debounce(async () => {
		await branchService.refresh().catch((error) => {
			console.error("Failed to refresh remote branches:", error);
		});
	}, 500);

	// Refresh on git fetch events
	$effect(() =>
		gitService.onFetch(data.projectId, () => {
			debouncedBaseBranchRefresh();
			debouncedRemoteBranchRefresh();
		}),
	);

	// Refresh when branch data changes
	$effect(() => {
		if (baseBranch || modeQuery.response) debouncedRemoteBranchRefresh();
	});

	// Auto-fetch setup
	async function fetchRemoteForProject() {
		await baseBranchService.fetchFromRemotes(projectId, "auto");
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
			goto("/onboarding");
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
					"Just FYI, this project is already open in another window",
					"There might be some unexpected behavior if you open it in multiple windows",
				);
			}

			if (info.db_error) {
				showError("The database was corrupted", info.db_error);
			}

			if (info.headsup && localStorage.getItem(dontShowAgainKey) !== "1") {
				showWarning("Important PSA", info.headsup, {
					label: "Don't show again",
					onClick: (dismiss) => {
						localStorage.setItem(dontShowAgainKey, "1");
						dismiss();
					},
				});
			}
		} catch (error: unknown) {
			posthog.captureOnboarding(OnboardingEvent.SetProjectActiveFailed);
			showError("Failed to set the project active", error);
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

	// =============================================================================
	// IRC PROJECT CHANNEL
	// =============================================================================

	const ircApiService = inject(IRC_API_SERVICE);
	const workingFilesBroadcast = inject(WORKING_FILES_BROADCAST);

	// Extract primitive values via $derived so the effect only re-runs when
	// the actual IRC-relevant settings change, not on every settings store emit.
	const ircEnabled = $derived(
		($settingsStore?.featureFlags?.irc && $settingsStore?.irc?.connection?.enabled) ?? false,
	);
	const ircProjectChannelSetting = $derived($settingsStore?.irc?.projectChannel);
	const projectTitle = $derived(currentProject?.title);

	$effect(() => {
		if (!ircEnabled || !projectTitle) return;

		const channel =
			ircProjectChannelSetting !== null && ircProjectChannelSetting !== undefined
				? ircProjectChannelSetting
				: projectChannel(projectTitle);

		const botsChannel = `${channel}/bots`;

		ircApiService.autoJoin({ channel });
		ircApiService.autoJoin({ channel: botsChannel });
		workingFilesBroadcast.start(projectId, botsChannel);

		return () => {
			ircApiService.autoLeave({ channel });
			ircApiService.autoLeave({ channel: botsChannel });
			workingFilesBroadcast.stop();
		};
	});

	// Cleanup on destroy
	onDestroy(() => {
		clearFetchInterval();
	});
</script>

<ProjectSettingsShortcutHandler {projectId} />
<ProjectShortcutHandler />

<ReduxResult {projectId} result={combineResults(baseBranchQuery.result, modeQuery.result)}>
	{#snippet children([baseBranch, mode], { projectId })}
		{#if !baseBranch}
			<NoBaseBranch {projectId} />
		{:else if baseBranch}
			{#if mode.type === "OpenWorkspace" || mode.type === "Edit" || ($settingsStore?.featureFlags.singleBranch && mode.subject.branchName)}
				<div class="view-wrap" role="group" ondragover={(e) => e.preventDefault()}>
					<AppLayout {projectId} sidebarDisabled={mode.type === "Edit"}>
						{@render pageChildren()}
					</AppLayout>
				</div>
			{:else if mode.type === "OutsideWorkspace"}
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

<IrcChatWindow {projectId} />

<AnalyticsMonitor {projectId} />

<style>
	.view-wrap {
		display: flex;
		position: relative;
		width: 100%;
	}
</style>
