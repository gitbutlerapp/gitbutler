<script lang="ts">
	import { goto } from "$app/navigation";
	import ProjectNameLabel from "$components/shared/ProjectNameLabel.svelte";
	import ProjectSwitcher from "$components/shared/ProjectSwitcher.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import gerritLogoSvg from "$lib/assets/gerrit-logo.svg?raw";
	import { getBestRemote, getBranchRemoteFromRef } from "$lib/branches/branchUtils";
	import { projectLandDirectly } from "$lib/config/config";
	import { GIT_CONFIG_SERVICE } from "$lib/config/gitConfigService";
	import { classify, type ClassifiedError } from "$lib/error/errorClassification";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { combineResults } from "$lib/state/helpers";
	import { OnboardingEvent, POSTHOG_WRAPPER } from "$lib/telemetry/posthog";
	import { unique } from "$lib/utils/array";
	import { inject } from "@gitbutler/core/context";
	import {
		Button,
		CardGroup,
		Checkbox,
		InfoMessage,
		Link,
		Select,
		SelectItem,
		Spacer,
		TestId,
		Toggle,
	} from "@gitbutler/ui-svelte";
	import type { RemoteBranchInfo } from "$lib/baseBranch/baseBranch";
	import type { LocalTargetBranch } from "$lib/branches/branchEndpoints";

	interface Props {
		projectId: string;
		projectName: string;
		remoteBranches: RemoteBranchInfo[];
		localBranches: LocalTargetBranch[];
		onBranchSelected: (
			branch: [branchName: string, pushRemote: string | undefined],
		) => Promise<void>;
		onOpenProject: () => Promise<boolean>;
		/**
		 * Why the target needs choosing again, shown above the branch picker. Replacing a target
		 * that was set before keeps the push remote it had, which project settings can change,
		 * and leaving means switching projects rather than removing this one.
		 */
		notice?: string;
	}

	const {
		projectId,
		projectName,
		remoteBranches,
		localBranches,
		onBranchSelected,
		onOpenProject,
		notice,
	}: Props = $props();

	const posthog = inject(POSTHOG_WRAPPER);
	const gitConfig = inject(GIT_CONFIG_SERVICE);

	const gbConfig = $derived(gitConfig.gbConfig(projectId));
	const gerritMode = $derived(gbConfig.response?.gitbutlerGerritMode ?? false);

	const landDirectly = $derived(projectLandDirectly(projectId));

	let loading = $state<boolean>(false);
	let targetWasSet = $state(false);
	let targetError = $state<ClassifiedError>();

	// split all the branches by the first '/' and gather the unique remote names
	// then turn remotes into an array of objects with a 'name' and 'value' key
	const remotes = $derived(
		unique(remoteBranches.map((b) => getBranchRemoteFromRef(b.name))).filter(
			(r): r is string => !!r,
		),
	);

	let selectedBranch = $state<LocalTargetBranch | undefined>(undefined);
	const defaultBranch = $derived(
		localBranches.find((branch) => branch.isTarget) ??
			localBranches.find((branch) => branch.name === "main") ??
			localBranches.find((branch) => branch.name === "master") ??
			localBranches[0],
	);
	const branch = $derived(selectedBranch ?? defaultBranch);

	let selectedRemote = $state<string | undefined>(undefined);
	const defaultRemote = $derived(getBestRemote(remotes));
	const remote = $derived(selectedRemote ?? defaultRemote);
	// The stored push remote can't be read while the target is unresolved, so it's left as is.
	const replacingTarget = $derived(notice !== undefined);

	async function onSetTargetClick() {
		if (!branch || loading) return;
		loading = true;
		targetError = undefined;
		if (!targetWasSet) {
			posthog.captureOnboarding(OnboardingEvent.ProjectSetupContinue, undefined, {
				landDirectly: $landDirectly,
			});
			try {
				await onBranchSelected([branch.refName, replacingTarget ? undefined : remote]);
				targetWasSet = true;
			} catch (error) {
				const classified = classify(error);
				if (classified.severity !== "silent") targetError = classified;
				loading = false;
				return;
			}
		}
		if (await onOpenProject()) return;
		loading = false;
	}

	const projectsService = inject(PROJECTS_SERVICE);
	async function deleteProjectAndGoBack() {
		await projectsService.deleteProject(projectId);
		await projectsService.fetchProjects();
		await goto("/");
	}

	const itSmellsLikeGerrit = $derived(projectsService.areYouGerritKiddingMe(projectId));
	const projectIsGerrit = $derived(projectsService.isGerritProject(projectId));
</script>

<div class="project-setup">
	<div class="stack-v gap-4">
		<ProjectNameLabel {projectName} />
		<h1 class="text-serif-42">Configure your <i>workspace</i></h1>
	</div>

	{#if notice}
		<InfoMessage style="warning">
			{#snippet content()}
				{notice}
			{/snippet}
		</InfoMessage>
	{/if}

	<div class="project-setup__fields">
		<div class="project-setup__field-wrap" data-testid={TestId.ProjectSetupPageTargetBranchSelect}>
			<Select
				value={branch?.refName}
				options={localBranches.map((b) => ({ label: b.name, value: b.refName }))}
				disabled={loading || targetWasSet}
				wide
				onselect={(value) => {
					selectedBranch = localBranches.find((branch) => branch.refName === value);
				}}
				label="Target branch"
				searchable
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={item.value === branch?.refName} {highlighted}>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>

			<p class="text-12 text-body project-setup__field-caption">
				Your main "production" branch, typically <code class="code-string">master</code> or
				<code class="code-string">main</code>. Its configured upstream is used for pulling.
				<br />
				<Link href="https://docs.gitbutler.com/overview#target-branch">Learn more</Link>
			</p>
		</div>

		{#if remotes.length > 1 && !replacingTarget}
			<div class="project-setup__field-wrap">
				<Select
					value={remote}
					options={remotes.map((r) => ({ label: r, value: r }))}
					disabled={loading || targetWasSet}
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
				<!-- Fail silently to detect the gerritness of a project -->
				<div></div>
			{/snippet}
			{#snippet children([isGerrit])}
				{#if isGerrit}
					<CardGroup.Item standalone labelFor="gerritToggle">
						{#snippet iconSide()}
							{@html gerritLogoSvg}
						{/snippet}
						{#snippet title()}
							Enable Gerrit project
						{/snippet}
						{#snippet caption()}
							It looks like this project might be a Gerrit project.
							<br />
							Do you want to enable Gerrit mode?
							<br />
							You can adjust this later in the project settings if needed.
						{/snippet}
						{#snippet actions()}
							<Toggle
								id="gerritToggle"
								checked={gerritMode}
								onclick={() => {
									gitConfig.setGerritMode(projectId, !gerritMode);
								}}
							/>
						{/snippet}
					</CardGroup.Item>
				{/if}
			{/snippet}
		</ReduxResult>
	</div>

	<label for="landDirectly" class="land-directly">
		<Checkbox name="landDirectly" small bind:checked={$landDirectly} />
		<span class="text-12 clr-text-2">Push to main / Skip pull requests mode</span>
	</label>

	{#if targetError}
		{@const error = targetError}
		<InfoMessage style={error.severity === "warning" ? "warning" : "danger"}>
			{#snippet content()}
				{error.userMessage ?? error.message}
			{/snippet}
		</InfoMessage>
	{/if}

	<div class="action-buttons">
		<!-- Cancelling removes the project and its GitButler data, only right for one just added. -->
		{#if !replacingTarget}
			<Button kind="outline" disabled={loading} onclick={deleteProjectAndGoBack}>Cancel</Button>
		{/if}
		<Button
			style="pop"
			{loading}
			disabled={!branch}
			onclick={onSetTargetClick}
			icon="chevron-right"
			testId={TestId.ProjectSetupPageTargetContinueButton}
			id="set-base-branch"
		>
			{targetWasSet ? "Open project" : "Let's go"}
		</Button>
	</div>

	{#if replacingTarget}
		<Spacer dotted margin={0} />
		<ProjectSwitcher {projectId} />
	{/if}
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
		gap: 20px;
	}

	.project-setup__field-wrap {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.project-setup__field-caption {
		width: 90%;
		color: var(--text-2);
	}

	.land-directly {
		display: flex;
		align-items: center;
		gap: 8px;
		cursor: pointer;
	}

	.action-buttons {
		display: flex;
		justify-content: flex-end;
		width: 100%;
		gap: 8px;
	}
</style>
