<script lang="ts">
	import { Project, ProjectService } from '$lib/backend/projects';
	import AiPromptSelect from '$lib/components/AIPromptSelect.svelte';
	import Button from '$lib/components/Button.svelte';
	import Link from '$lib/components/Link.svelte';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import Spacer from '$lib/components/Spacer.svelte';
	import Toggle from '$lib/components/Toggle.svelte';
	import WelcomeSigninAction from '$lib/components/WelcomeSigninAction.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { projectAiGenAutoBranchNamingEnabled } from '$lib/config/config';
	import Section from '$lib/settings/Section.svelte';
	import { UserService } from '$lib/stores/user';
	import { getContext } from '$lib/utils/context';
	import * as toasts from '$lib/utils/toasts';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { PUBLIC_API_BASE_URL } from '$env/static/public';

	const userService = getContext(UserService);
	const projectService = getContext(ProjectService);
	const project = getContext(Project);
	const user = userService.user;

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
</script>

{#if !$user}
	<WelcomeSigninAction />
	<Spacer />
{/if}

<Section spacer>
	<svelte:fragment slot="title">AI options</svelte:fragment>
	<svelte:fragment slot="description">
		GitButler supports the use of OpenAI and Anthropic to provide commit message and branch name
		generation. This works either through GitButler's API or in a bring your own key configuration
		and can be configured in the main preferences screen.
	</svelte:fragment>

	<div class="options">
		<SectionCard labelFor="aiGenEnabled" orientation="row">
			<svelte:fragment slot="title">Enable branch and commit message generation</svelte:fragment>
			<svelte:fragment slot="caption">
				If enabled, diffs will sent to OpenAI or Anthropic's servers when pressing the "Generate
				message" and "Generate branch name" button.
			</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle
					id="aiGenEnabled"
					checked={$aiGenEnabled}
					on:click={() => {
						$aiGenEnabled = !$aiGenEnabled;
						$aiGenAutoBranchNamingEnabled = $aiGenEnabled;
					}}
				/>
			</svelte:fragment>
		</SectionCard>

		<SectionCard labelFor="branchNameGen" disabled={!$aiGenEnabled} orientation="row">
			<svelte:fragment slot="title">Automatically generate branch names</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle
					id="branchNameGen"
					disabled={!$aiGenEnabled}
					checked={$aiGenAutoBranchNamingEnabled}
					on:click={() => ($aiGenAutoBranchNamingEnabled = !$aiGenAutoBranchNamingEnabled)}
				/>
			</svelte:fragment>
		</SectionCard>
	</div>

	<SectionCard>
		<svelte:fragment slot="title">Custom prompts</svelte:fragment>

		<AiPromptSelect promptUse="commits" />
		<AiPromptSelect promptUse="branches" />

		<Spacer margin={8} />

		<p class="text-base-body-12">
			You can apply your own custom prompts to the project. By default, the project uses GitButler
			prompts, but you can create your own prompts in the general settings.
		</p>
		<Button style="ghost" outline icon="edit-text" on:click={async () => await goto('/settings/ai')}
			>Customize prompts</Button
		>
	</SectionCard>
</Section>

{#if $user?.role === 'admin'}
	<Section spacer>
		<svelte:fragment slot="title">Full data synchronization</svelte:fragment>

		<SectionCard labelFor="historySync" orientation="row">
			<svelte:fragment slot="caption">
				Sync my history, repository and branch data for backup, sharing and team features.
			</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle
					id="historySync"
					checked={project.api?.sync || false}
					on:click={async (e) => await onSyncChange(!!e.detail)}
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

<style lang="postcss">
	.options {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.api-link {
		display: flex;
		justify-content: flex-end;
	}
</style>
