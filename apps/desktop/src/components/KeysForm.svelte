<script lang="ts">
	import CredentialCheck from '$components/CredentialCheck.svelte';
	import Link from '$components/Link.svelte';
	import ProjectNameLabel from '$components/ProjectNameLabel.svelte';
	import RadioButton from '$components/RadioButton.svelte';
	import Section from '$components/Section.svelte';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { showError } from '$lib/notifications/toasts';
	import { ProjectsService, type Key, type KeyType, Project } from '$lib/project/projects';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import { onMount } from 'svelte';

	const project = $state(getContext(Project));

	const baseBranch = getContextStore(BaseBranch);
	const projectsService = getContext(ProjectsService);

	interface Props {
		// Used by credential checker before target branch set
		remoteName?: string;
		branchName?: string;
		showProjectName?: boolean;
		disabled?: boolean;
	}

	const {
		remoteName = '',
		branchName = '',
		showProjectName = false,
		disabled = false
	}: Props = $props();

	let credentialCheck = $state<CredentialCheck>();

	let selectedType: KeyType = $state(
		typeof project.preferred_key === 'string' ? project.preferred_key : 'local'
	);

	let privateKeyPath = $state(
		typeof project.preferred_key === 'string' ? '' : project.preferred_key.local.private_key_path
	);

	function setLocalKey() {
		if (privateKeyPath.trim().length === 0) return;
		updateKey({
			preferred_key: {
				local: {
					private_key_path: privateKeyPath.trim()
				}
			}
		});
	}

	async function updateKey(detail: { preferred_key: Key }) {
		try {
			project.preferred_key = detail.preferred_key;
			projectsService.updateProject(project);
		} catch (err: any) {
			showError('Failed to update key', err);
		}
	}

	let form = $state<HTMLFormElement>();

	function onFormChange(form: HTMLFormElement) {
		credentialCheck?.reset();
		const formData = new FormData(form);
		selectedType = formData.get('credentialType') as KeyType;
		if (selectedType !== 'local') {
			updateKey({ preferred_key: selectedType });
		} else {
			setLocalKey();
		}
	}

	onMount(async () => {
		if (form) {
			form.credentialType.value = selectedType;
		}
	});
</script>

<Section>
	{#snippet top()}
		{#if showProjectName}<ProjectNameLabel projectName={project.title} />{/if}
	{/snippet}
	{#snippet title()}
		Git authentication
	{/snippet}
	{#snippet description()}
		Configure the authentication flow for GitButler when authenticating with your Git remote
		provider.
	{/snippet}

	<form
		class="git-radio"
		class:disabled
		bind:this={form}
		onchange={(e) => onFormChange(e.currentTarget)}
	>
		<SectionCard roundedBottom={false} orientation="row" labelFor="git-executable">
			{#snippet title()}
				Use a Git executable <span style="color: var(--clr-text-2)">(default)</span>
			{/snippet}

			{#snippet caption()}
				{#if selectedType === 'systemExecutable'}
					Git executable must be present on your PATH
				{/if}
			{/snippet}

			{#snippet actions()}
				<RadioButton name="credentialType" value="systemExecutable" id="git-executable" />
			{/snippet}
		</SectionCard>

		<SectionCard
			roundedTop={false}
			roundedBottom={false}
			bottomBorder={selectedType !== 'local'}
			orientation="row"
			labelFor="credential-local"
		>
			{#snippet title()}
				Use existing SSH key
			{/snippet}

			{#snippet actions()}
				<RadioButton name="credentialType" id="credential-local" value="local" />
			{/snippet}

			{#snippet caption()}
				{#if selectedType === 'local'}
					Add the path to an existing SSH key that GitButler can use.
				{/if}
			{/snippet}
		</SectionCard>

		{#if selectedType === 'local'}
			<SectionCard topDivider roundedTop={false} roundedBottom={false} orientation="row">
				<div class="inputs-group">
					<Textbox
						label="Path to private key"
						placeholder="for example: ~/.ssh/id_rsa"
						bind:value={privateKeyPath}
					/>
				</div>
			</SectionCard>
		{/if}

		<SectionCard
			roundedTop={false}
			roundedBottom={false}
			orientation="row"
			labelFor="credential-helper"
		>
			{#snippet title()}
				Use a Git credentials helper
			{/snippet}

			{#snippet caption()}
				{#if selectedType === 'gitCredentialsHelper'}
					GitButler will use the system's git credentials helper.
					<Link target="_blank" rel="noreferrer" href="https://git-scm.com/doc/credential-helpers">
						Learn more
					</Link>
				{/if}
			{/snippet}

			{#snippet actions()}
				<RadioButton name="credentialType" value="gitCredentialsHelper" id="credential-helper" />
			{/snippet}
		</SectionCard>

		<SectionCard roundedTop={false} orientation="row">
			<CredentialCheck
				bind:this={credentialCheck}
				projectId={project.id}
				remoteName={remoteName || $baseBranch?.remoteName}
				branchName={branchName || $baseBranch?.shortName}
			/>
		</SectionCard>
	</form>
</Section>

<style lang="postcss">
	.inputs-group {
		display: flex;
		flex-direction: column;
		gap: 16px;
		width: 100%;
	}

	.git-radio {
		display: flex;
		flex-direction: column;

		&.disabled {
			pointer-events: none;
			opacity: 0.5;
		}
	}
</style>
