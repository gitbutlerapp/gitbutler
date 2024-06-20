<script lang="ts">
	import { AuthService } from '$lib/backend/auth';
	import { ProjectService, type Key, type KeyType, Project } from '$lib/backend/projects';
	import Button from '$lib/components/Button.svelte';
	import CredentialCheck from '$lib/components/CredentialCheck.svelte';
	import Link from '$lib/components/Link.svelte';
	import ProjectNameLabel from '$lib/components/ProjectNameLabel.svelte';
	import RadioButton from '$lib/components/RadioButton.svelte';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import TextBox from '$lib/components/TextBox.svelte';
	import { showError } from '$lib/notifications/toasts';
	import Section from '$lib/settings/Section.svelte';
	import { copyToClipboard } from '$lib/utils/clipboard';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { openExternalUrl } from '$lib/utils/url';
	import { BaseBranch } from '$lib/vbranches/types';
	import { onMount } from 'svelte';

	const project = getContext(Project);

	const authService = getContext(AuthService);
	const baseBranch = getContextStore(BaseBranch);
	const projectService = getContext(ProjectService);

	// Used by credential checker before target branch set
	export let remoteName = '';
	export let branchName = '';
	export let showProjectName = false;
	export let disabled = false;

	let sshKey = '';
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
		sshKey = await authService.getPublicKey();
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
			bottomBorder={selectedType !== 'generated'}
			orientation="row"
			labelFor="credential-generated"
		>
			<svelte:fragment slot="title">Use locally generated SSH key</svelte:fragment>

			<svelte:fragment slot="actions">
				<RadioButton name="credentialType" id="credential-generated" value="generated" />
			</svelte:fragment>

			<svelte:fragment slot="caption">
				{#if selectedType === 'generated'}
					GitButler will use a locally generated SSH key. For this to work you need to add the
					following public key to your Git remote provider:
				{/if}
			</svelte:fragment>
		</SectionCard>

		{#if selectedType === 'generated'}
			<SectionCard topDivider roundedTop={false} roundedBottom={false}>
				<TextBox id="sshKey" readonly bind:value={sshKey} wide />
				<div class="row-buttons">
					<Button style="pop" kind="solid" icon="copy" on:mousedown={() => copyToClipboard(sshKey)}>
						Copy to Clipboard
					</Button>
					<Button
						style="ghost"
						outline
						icon="open-link"
						on:mousedown={() => {
							openExternalUrl('https://github.com/settings/ssh/new');
						}}
					>
						Add key to GitHub
					</Button>
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

	.row-buttons {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
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
