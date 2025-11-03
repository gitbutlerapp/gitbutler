<script lang="ts">
	import ProjectNameLabel from '$components/ProjectNameLabel.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { OnboardingEvent, POSTHOG_WRAPPER } from '$lib/analytics/posthog';
	import { BACKEND } from '$lib/backend';
	import { GIT_CONFIG_SERVICE } from '$lib/config/gitConfigService';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { combineResults } from '$lib/state/helpers';
	import { unique } from '$lib/utils/array';
	import { getBestBranch, getBestRemote, getBranchRemoteFromRef } from '$lib/utils/branch';
	import { inject } from '@gitbutler/core/context';
	import { Button, Select, SelectItem, TestId, Link, Icon } from '@gitbutler/ui';
	import { slide } from 'svelte/transition';
	import type { RemoteBranchInfo } from '$lib/baseBranch/baseBranch';

	interface Props {
		projectId: string;
		projectName: string;
		remoteBranches: RemoteBranchInfo[];
		onBranchSelected?: (branch: string[]) => void;
	}

	const { projectId, projectName, remoteBranches, onBranchSelected }: Props = $props();

	const backend = inject(BACKEND);
	const posthog = inject(POSTHOG_WRAPPER);
	const gbConfig = inject(GIT_CONFIG_SERVICE);

	let loading = $state<boolean>(false);
	let showMoreInfo = $state<boolean>(false);

	// split all the branches by the first '/' and gather the unique remote names
	// then turn remotes into an array of objects with a 'name' and 'value' key
	const remotes = $derived(
		unique(remoteBranches.map((b) => getBranchRemoteFromRef(b.name))).filter(
			(r): r is string => !!r
		)
	);

	let selectedBranch = $state<RemoteBranchInfo | undefined>(undefined);
	const defaultBranch = $derived(getBestBranch(remoteBranches.slice()));
	const branch = $derived(selectedBranch ?? defaultBranch);

	let selectedRemote = $state<string | undefined>(undefined);
	const defaultRemote = $derived(
		(branch && getBranchRemoteFromRef(branch.name)) ?? getBestRemote(remotes)
	);
	const remote = $derived(selectedRemote ?? defaultRemote);

	async function onSetTargetClick() {
		if (!branch || !remote) return;
		posthog.captureOnboarding(OnboardingEvent.ProjectSetupContinue);
		onBranchSelected?.([branch.name, remote]);
	}

	const projectsService = inject(PROJECTS_SERVICE);
	async function deleteProjectAndGoBack() {
		await projectsService.deleteProject(projectId);
	}

	const itSmellsLikeGerrit = $derived(projectsService.areYouGerritKiddingMe(projectId));
	const projectIsGerrit = $derived(projectsService.isGerritProject(projectId));
</script>

<div class="project-setup">
	<div class="stack-v gap-4">
		<ProjectNameLabel {projectName} />
		<h1 class="text-serif-42">Configure your <i>workspace</i></h1>
	</div>

	<div class="project-setup__fields">
		<div class="project-setup__field-wrap" data-testid={TestId.ProjectSetupPageTargetBranchSelect}>
			<Select
				value={branch?.name}
				options={remoteBranches.map((b) => ({ label: b.name, value: b.name }))}
				wide
				onselect={(value) => {
					selectedBranch = { name: value };
				}}
				label="Target branch"
				searchable
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={item.value === branch?.name} {highlighted}>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>

			<p class="text-12 text-body clr-text-2">
				Your main "production" branch, typically <code class="code-string">origin/master</code> or
				<code class="code-string">upstream/main</code>.
				<Link href="https://docs.gitbutler.com/overview#target-branch">Learn more</Link>
			</p>
		</div>

		{#if remotes.length > 1}
			<div class="project-setup__field-wrap">
				<Select
					value={remote}
					options={remotes.map((r) => ({ label: r, value: r }))}
					onselect={(value) => {
						const newSelectedRemote = remotes.find((r) => r === value);
						selectedRemote = newSelectedRemote ?? remote;
					}}
				>
					{#snippet itemSnippet({ item, highlighted })}
						<SelectItem selected={item.value === remote} {highlighted}>
							{item.label}
						</SelectItem>
					{/snippet}
				</Select>

				<p class="text-12 text-body clr-text-2">
					You have branches from multiple remotes. If you want to specify a remote for creating
					branches that is different from the remote that your target branch is on, change it here.
				</p>
			</div>
		{/if}

		<ReduxResult
			{projectId}
			result={combineResults(itSmellsLikeGerrit.result, projectIsGerrit.result)}
		>
			{#snippet error()}
				<!-- Fail silently when prompting for gerritness -->
				<div></div>
			{/snippet}
			{#snippet children([isGerrit, isActuallyGerrit])}
				{#if isGerrit && !isActuallyGerrit}
					<p class="text-12">
						Is this project a gerrit project? If so, please confirm that in order to enable <i
							>Gerrit Mode to the Xtreme™</i
						>.
						<br />
						Otherwise, ignore this message.
					</p>
					<div>
						<Button
							onclick={() => {
								gbConfig.setGerritMode(projectId, true);
							}}
						>
							Yup, it's a Gerrit project
						</Button>
					</div>
				{:else if isActuallyGerrit}
					<p class="text-12">
						Cool, this is a Gerrit project! GitButler will adjust its behavior accordingly.
					</p>
				{/if}
			{/snippet}
		</ReduxResult>
	</div>

	<div
		class="project-setup__info"
		role="presentation"
		onclick={() => (showMoreInfo = !showMoreInfo)}
	>
		<div class="project-setup__fold-icon" class:rotate-icon={showMoreInfo}>
			<Icon name="chevron-right" />
		</div>

		<div class="stack-v gap-6 full-width">
			<div class="project-setup__info__title">
				<svg
					width="16"
					height="13"
					viewBox="0 0 16 13"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
				>
					<path
						d="M2 12L3.5 7.5M14 12L12.5 7.5M12.5 7.5L11 3H5L3.5 7.5M12.5 7.5H3.5"
						stroke="#D96842"
						stroke-width="1.5"
					/>
					<path
						d="M1.24142 3H14.7586C14.8477 3 14.8923 2.89229 14.8293 2.82929L13.0293 1.02929C13.0105 1.01054 12.9851 1 12.9586 1H3.04142C3.0149 1 2.98946 1.01054 2.97071 1.02929L1.17071 2.82929C1.10771 2.89229 1.15233 3 1.24142 3Z"
						fill="#FF9774"
						stroke="#FF9774"
						stroke-width="1.5"
					/>
				</svg>

				<h3 class="text-13 text-body text-semibold">
					GitButler switches your repo to gitbutler/workspace
				</h3>
			</div>

			{#if showMoreInfo}
				<p class="text-12 text-body" transition:slide={{ duration: 200 }}>
					In order to support working on multiple branches simultaneously, GitButler creates and
					automatically manages a special branch <span class="text-bold">gitbutler/workspace</span>.
					You can always switch back and forth as needed between normal git branches and the
					Gitbutler workspace.
					<Link href="https://docs.gitbutler.com/features/branch-management/integration-branch"
						>Learn more</Link
					>
				</p>
			{/if}
		</div>
	</div>

	<div class="action-buttons">
		<Button kind="outline" onclick={deleteProjectAndGoBack}>Cancel</Button>
		<Button
			style="pop"
			{loading}
			onclick={onSetTargetClick}
			icon="chevron-right-small"
			testId={TestId.ProjectSetupPageTargetContinueButton}
			id="set-base-branch"
		>
			{#if backend.platformName === 'windows'}
				Let's go
			{:else}
				Continue
			{/if}
		</Button>
	</div>
</div>

<style lang="postcss">
	.project-setup {
		display: flex;
		flex-direction: column;
		gap: 20px;
	}

	.project-setup__fields {
		display: flex;
		flex-direction: column;
		padding-bottom: 10px;
		gap: 20px;
	}

	.project-setup__field-wrap {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.action-buttons {
		display: flex;
		justify-content: flex-end;
		width: 100%;
		gap: 8px;
	}

	/* BANNER */
	.project-setup__info {
		display: flex;
		padding: 14px 16px;
		gap: 8px;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1-muted);
		cursor: pointer;

		&:hover {
			& .project-setup__fold-icon {
				color: var(--clr-text-2);
			}
		}
	}

	.project-setup__info__title {
		display: inline;
		width: 100%;
		gap: 8px;

		svg {
			display: inline;
			margin-right: 8px;
			float: left;
			transform: translateY(4px);
		}
	}

	.project-setup__fold-icon {
		display: flex;
		align-self: flex-start;
		padding-top: 2px;
		color: var(--clr-text-3);
		transition:
			transform var(--transition-medium),
			color var(--transition-fast);

		&.rotate-icon {
			transform: rotate(90deg);
		}
	}
</style>
