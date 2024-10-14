<script lang="ts">
	import { ProjectService, type Key, type KeyType, Project } from '$lib/backend/projects';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import { showError } from '$lib/notifications/toasts';
	import Section from '$lib/settings/Section.svelte';
	import CredentialCheck from '$lib/shared/CredentialCheck.svelte';
	import Link from '$lib/shared/Link.svelte';
	import ProjectNameLabel from '$lib/shared/ProjectNameLabel.svelte';
	import RadioButton from '$lib/shared/RadioButton.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import { onMount } from 'svelte';

	const project = getContext(Project);

	const baseBranch = getContextStore(BaseBranch);
	const projectService = getContext(ProjectService);

	// Used by credential checker before target branch set
	export let remoteName = '';
	export let branchName = '';
	export let showProjectName = false;
	export let disabled = false;

	let credentialCheck: CredentialCheck;

	let selectedType: KeyType =
		typeof project.preferred_key === 'string' ? project.preferred_key : 'local';

	let privateKeyPath =
		typeof project.preferred_key === 'string' ? '' : project.preferred_key.local.private_key_path;

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
			projectService.updateProject(project);
		} catch (err: any) {
			showError('Failed to update key', err);
		}
	}

	let form: HTMLFormElement;

	function onFormChange(form: HTMLFormElement) {
		credentialCheck.reset();
		const formData = new FormData(form);
		selectedType = formData.get('credentialType') as KeyType;
		if (selectedType !== 'local') {
			updateKey({ preferred_key: selectedType });
		} else {
			setLocalKey();
		}
	}

	onMount(async () => {
		form.credentialType.value = selectedType;
	});
</script>

<Section>
	<svelte:fragment slot="top"
		>{#if showProjectName}<ProjectNameLabel projectName={project.title} />{/if}</svelte:fragment
	>
	<svelte:fragment slot="title">Git authentication</svelte:fragment>
	<svelte:fragment slot="description">
		Configure the authentication flow for GitButler when authenticating with your Git remote
		provider.
	</svelte:fragment>

	<form
		class="git-radio"
		class:disabled
		bind:this={form}
		on:change={(e) => onFormChange(e.currentTarget)}
	>
		<SectionCard roundedBottom={false} orientation="row" labelFor="git-executable">
			<svelte:fragment slot="title"
				>Use a Git executable <span style="color: var(--clr-text-2)">(default)</span
				></svelte:fragment
			>

			<svelte:fragment slot="caption">
				{#if selectedType === 'systemExecutable'}
					Git executable must be present on your PATH
				{/if}
			</svelte:fragment>

			<svelte:fragment slot="actions">
				<RadioButton name="credentialType" value="systemExecutable" id="git-executable" />
			</svelte:fragment>
		</SectionCard>

		<SectionCard
			roundedTop={false}
			roundedBottom={false}
			bottomBorder={selectedType !== 'local'}
			orientation="row"
			labelFor="credential-local"
		>
			<svelte:fragment slot="title">Use existing SSH key</svelte:fragment>

			<svelte:fragment slot="actions">
				<RadioButton name="credentialType" id="credential-local" value="local" />
			</svelte:fragment>

			<svelte:fragment slot="caption">
				{#if selectedType === 'local'}
					Add the path to an existing SSH key that GitButler can use.
				{/if}
			</svelte:fragment>
		</SectionCard>

		{#if selectedType === 'local'}
			<SectionCard topDivider roundedTop={false} roundedBottom={false} orientation="row">
				<div class="inputs-group">
					<TextBox
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
			<svelte:fragment slot="title">Use a Git credentials helper</svelte:fragment>

			<svelte:fragment slot="caption">
				{#if selectedType === 'gitCredentialsHelper'}
					GitButler will use the system's git credentials helper.
					<Link target="_blank" rel="noreferrer" href="https://git-scm.com/doc/credential-helpers">
						Learn more
					</Link>
				{/if}
			</svelte:fragment>

			<svelte:fragment slot="actions">
				<RadioButton name="credentialType" value="gitCredentialsHelper" id="credential-helper" />
			</svelte:fragment>
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
