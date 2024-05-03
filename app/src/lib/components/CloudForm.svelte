<script lang="ts">
	import Button from './Button.svelte';
	import InfoMessage from './InfoMessage.svelte';
	import SectionCard from './SectionCard.svelte';
	import Select from './Select.svelte';
	import SelectItem from './SelectItem.svelte';
	import WelcomeSigninAction from './WelcomeSigninAction.svelte';
	import { Project, ProjectService } from '$lib/backend/projects';
	import Link from '$lib/components/Link.svelte';
	import Spacer from '$lib/components/Spacer.svelte';
	import Toggle from '$lib/components/Toggle.svelte';
	import Section from '$lib/components/settings/Section.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { projectAiGenAutoBranchNamingEnabled } from '$lib/config/config';
	import { UserService } from '$lib/stores/user';
	import { getContext, getContextStore } from '$lib/utils/context';
	import * as toasts from '$lib/utils/toasts';
	import { getRemoteBranches } from '$lib/vbranches/baseBranch';
	import { BranchController } from '$lib/vbranches/branchController';
	import { BaseBranch } from '$lib/vbranches/types';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import { onMount } from 'svelte';
	import { PUBLIC_API_BASE_URL } from '$env/static/public';

	const vbranchService = getContext(VirtualBranchService);
	const userService = getContext(UserService);
	const projectService = getContext(ProjectService);
	const project = getContext(Project);
	const branchController = getContext(BranchController);
	const baseBranch = getContextStore(BaseBranch);
	const user = userService.user;
	const activeBranches = vbranchService.activeBranches;

	const aiGenEnabled = projectAiGenEnabled(project.id);
	const aiGenAutoBranchNamingEnabled = projectAiGenAutoBranchNamingEnabled(project.id);

	onMount(async () => {
		if (!project?.api) return;
		if (!$user) return;
		const cloudProject = await projectService.getCloudProject(
			$user.access_token,
			project.api.repository_id
		);
		if (cloudProject === project.api) return;
		project.api = { ...cloudProject, sync: project.api.sync };
		projectService.updateProject(project);
	});

	async function onSyncChange(sync: boolean) {
		if (!$user) return;
		try {
			const cloudProject =
				project.api ??
				(await projectService.createCloudProject($user.access_token, {
					name: project.title,
					description: project.description,
					uid: project.id
				}));
			project.api = { ...cloudProject, sync };
			projectService.updateProject(project);
		} catch (error) {
			console.error(`Failed to update project sync status: ${error}`);
			toasts.error('Failed to update project sync status');
		}
	}

	function aiGenToggle() {
		$aiGenEnabled = !$aiGenEnabled;
		$aiGenAutoBranchNamingEnabled = $aiGenEnabled;
	}

	function aiGenBranchNamesToggle() {
		$aiGenAutoBranchNamingEnabled = !$aiGenAutoBranchNamingEnabled;
	}

	let selectedBranch = {name: $baseBranch.branchName};
	let selectedRemote = {name: $baseBranch.actualPushRemoteName()};
	let targetChangeDisabled = true;
	if ($activeBranches) {
		targetChangeDisabled = $activeBranches.length > 0;
	}
	let isSwitching = false;

	function uniqueRemotes(remoteBranches: { name: string }[]) {
		return Array.from(new Set(remoteBranches.map((b) => b.name.split('/')[0]))).map((r) => ({ name: r }));
	}

	async function onSetBaseBranchClick() {
		if (!selectedBranch) return;

		// while target is setting, display loading
		isSwitching = true;

		if(selectedRemote){
			await branchController
				.setTarget(selectedBranch.name, selectedRemote.name)
				.finally(() => {
					isSwitching = false;
				});
		} else {
			await branchController
				.setTarget(selectedBranch.name)
				.finally(() => {
					isSwitching = false;
				});
		}
	}
</script>

{#if !$user}
	<WelcomeSigninAction />
	<Spacer />
{/if}

<Section spacer>
		<SectionCard labelFor="targetBranch" orientation="column">
			<svelte:fragment slot="title">Current Target Branch</svelte:fragment>
			<svelte:fragment slot="caption">
				Your target branch is what you consider "production". This is where you
				want to integrate any branches that you create. Normally something like
				'origin/master' or 'upstream/main'.
			</svelte:fragment>

			<div class="inputs-group">
				{#if isSwitching}
					<InfoMessage filled outlined={false} style="pop" icon="info">
						<svelte:fragment slot="title">
							Switching target branch...
						</svelte:fragment>
					</InfoMessage>
				{:else}
					{#await getRemoteBranches(project.id)}
						loading remote branches...
					{:then remoteBranches}
						{#if remoteBranches.length == 0}
							<InfoMessage filled outlined={false} style="error" icon="error">
								<svelte:fragment slot="title">
									You don't have any remote branches.
								</svelte:fragment>
							</InfoMessage>
						{:else}
							<div class="inputs-row">
								<Select
									items={remoteBranches}
									bind:value={selectedBranch}
									itemId="name"
									labelId="name"
									disabled={targetChangeDisabled}
									wide={true}
								>
									<SelectItem slot="template" let:item let:selected {selected}>
										{item.name}
									</SelectItem>
								</Select>
								<Button disabled={targetChangeDisabled} on:click={onSetBaseBranchClick} style="{targetChangeDisabled ? 'neutral' : 'pop'}">Change Target Branch</Button>
							</div>

							{#if uniqueRemotes(remoteBranches).length > 1}
								Create branches on remote:
								<Select
									items={uniqueRemotes(remoteBranches)}
									bind:value={selectedRemote}
									itemId="name"
									labelId="name"
									disabled={targetChangeDisabled}
								>
									<SelectItem slot="template" let:item let:selected {selected}>
										{item.name}
									</SelectItem>
								</Select>
							{/if}

						{/if}
					{:catch}
						<InfoMessage filled outlined={true} style="error" icon="error">
							<svelte:fragment slot="title">
								We got an error trying to list your remote branches
							</svelte:fragment>
						</InfoMessage>
					{/await}
				{/if}

				{#if targetChangeDisabled}
				<InfoMessage filled outlined={false} icon="info">
					<svelte:fragment slot="title">
						You have active branches in your workspace. Please clear the workspace before switching the target branch.
					</svelte:fragment>
				</InfoMessage>
				{/if}
			</div>
		</SectionCard>
</Section>

<Section spacer>
	<svelte:fragment slot="title">AI options</svelte:fragment>
	<svelte:fragment slot="description">
		GitButler supports the use of OpenAI and Anthropic to provide commit message and branch name
		generation. This works either through GitButler's API or in a bring your own key configuration
		and can be configured in the main preferences screen.
	</svelte:fragment>

	<div class="options">
		<SectionCard labelFor="aiGenEnabled" on:click={aiGenToggle} orientation="row">
			<svelte:fragment slot="title">Enable branch and commit message generation</svelte:fragment>
			<svelte:fragment slot="caption">
				If enabled, diffs will sent to OpenAI or Anthropic's servers when pressing the "Generate
				message" and "Generate branch name" button.
			</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle id="aiGenEnabled" checked={$aiGenEnabled} on:change={aiGenToggle} />
			</svelte:fragment>
		</SectionCard>

		<SectionCard
			labelFor="branchNameGen"
			disabled={!$aiGenEnabled}
			on:click={aiGenBranchNamesToggle}
			orientation="row"
		>
			<svelte:fragment slot="title">Automatically generate branch names</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle
					id="branchNameGen"
					disabled={!$aiGenEnabled}
					checked={$aiGenAutoBranchNamingEnabled}
					on:change={aiGenBranchNamesToggle}
				/>
			</svelte:fragment>
		</SectionCard>
	</div>
</Section>

{#if $user?.role === 'admin'}
	<Section spacer>
		<svelte:fragment slot="title">Full data synchronization</svelte:fragment>

		<SectionCard
			labelFor="historySync"
			on:change={async (e) => await onSyncChange(e.detail)}
			orientation="row"
		>
			<svelte:fragment slot="caption">
				Sync my history, repository and branch data for backup, sharing and team features.
			</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle
					id="historySync"
					checked={project.api?.sync || false}
					on:change={async (e) => await onSyncChange(e.detail)}
				/>
			</svelte:fragment>
		</SectionCard>

		{#if project.api}
			<div class="api-link">
				<Link
					target="_blank"
					rel="noreferrer"
					href="{PUBLIC_API_BASE_URL}projects/{project.api?.repository_id}"
					>Go to GitButler Cloud Project</Link
				>
			</div>
		{/if}
	</Section>
{/if}

<style lang="post-css">
	.options {
		display: flex;
		flex-direction: column;
		gap: var(--size-8);
	}

	.api-link {
		display: flex;
		justify-content: flex-end;
	}
	.inputs-group {
		display: flex;
		flex-direction: column;
		gap: var(--size-16);
		width: 100%;
	}

	.inputs-row {
		display: flex;
		justify-content: space-between;
		gap: var(--size-16);
	}
</style>
