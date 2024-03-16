<script lang="ts">
	import CredentialCheck from './CredentialCheck.svelte';
	import RadioButton from './RadioButton.svelte';
	import SectionCard from './SectionCard.svelte';
	import TextBox from './TextBox.svelte';
	import { ProjectService, type Key, type KeyType, type Project } from '$lib/backend/projects';
	import Button from '$lib/components/Button.svelte';
	import Link from '$lib/components/Link.svelte';
	import { copyToClipboard } from '$lib/utils/clipboard';
	import { getContextByClass, getContextStoreByClass } from '$lib/utils/context';
	import * as toasts from '$lib/utils/toasts';
	import { openExternalUrl } from '$lib/utils/url';
	import { BaseBranch } from '$lib/vbranches/types';
	import { onMount } from 'svelte';
	import type { AuthService } from '$lib/backend/auth';

	export let authService: AuthService;
	export let project: Project;

	const baseBranch = getContextStoreByClass(BaseBranch);

	// Used by credential checker before target branch set
	export let remoteName = '';
	export let branchName = '';

	const projectService = getContextByClass(ProjectService);

	let credentialCheck: CredentialCheck;
	let sshKey = '';

	let selectedType: KeyType =
		typeof project.preferred_key == 'string' ? project.preferred_key : 'local';

	let privateKeyPath =
		typeof project.preferred_key == 'string' ? '' : project.preferred_key.local.private_key_path;

	let privateKeyPassphrase =
		typeof project.preferred_key == 'string' ? '' : project.preferred_key.local.passphrase;

	function setLocalKey() {
		if (privateKeyPath.trim().length == 0) return;
		updateKey({
			preferred_key: {
				local: {
					private_key_path: privateKeyPath.trim(),
					passphrase: privateKeyPassphrase || undefined
				}
			}
		});
	}

	async function updateKey(detail: { preferred_key: Key }) {
		try {
			projectService.updateProject({ ...project, ...detail });
		} catch (err: any) {
			toasts.error(err.message);
		}
	}

	let showPassphrase = false;

	let form: HTMLFormElement;

	function onFormChange(form: HTMLFormElement) {
		credentialCheck.reset();
		const formData = new FormData(form);
		selectedType = formData.get('credentialType') as KeyType;
		if (selectedType != 'local') {
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

<div class="git-auth-wrap">
	<h3 class="text-base-15 text-bold">Git Authentication</h3>
	<p class="text-base-body-12">
		Configure the authentication flow for GitButler when authenticating with your Git remote
		provider.
	</p>

	<form class="git-radio" bind:this={form} on:change={(e) => onFormChange(e.currentTarget)}>
		<SectionCard roundedBottom={false} orientation="row" labelFor="credential-default">
			<svelte:fragment slot="title">Auto detect</svelte:fragment>

			<svelte:fragment slot="actions">
				<RadioButton name="credentialType" id="credential-default" value="default" />
			</svelte:fragment>

			<svelte:fragment slot="caption">
				GitButler will attempt all available authentication flows automatically.
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
				Add the path to an existing SSH key that GitButler can use.
			</svelte:fragment>
		</SectionCard>

		{#if selectedType == 'local'}
			<SectionCard hasTopRadius={false} roundedTop={false} roundedBottom={false} orientation="row">
				<div class="inputs-group">
					<TextBox
						label="Path to private key"
						placeholder="for example: ~/.ssh/id_rsa"
						bind:value={privateKeyPath}
					/>

					<div class="input-with-button">
						<TextBox
							label="Passphrase (optional)"
							type={showPassphrase ? 'text' : 'password'}
							bind:value={privateKeyPassphrase}
							wide
						/>
						<Button
							size="large"
							color="neutral"
							kind="outlined"
							icon={showPassphrase ? 'eye-shown' : 'eye-hidden'}
							on:click={() => (showPassphrase = !showPassphrase)}
							width={150}
						>
							{showPassphrase ? 'Hide passphrase' : 'Show passphrase'}
						</Button>
					</div>
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
				GitButler will use a locally generated SSH key. For this to work you need to add the
				following public key to your Git remote provider:
			</svelte:fragment>
		</SectionCard>

		{#if selectedType === 'generated'}
			<SectionCard roundedTop={false} roundedBottom={false}>
				<TextBox id="sshKey" readonly selectall bind:value={sshKey} wide />
				<div class="row-buttons">
					<Button
						kind="filled"
						color="primary"
						icon="copy"
						on:mousedown={() => copyToClipboard(sshKey)}
					>
						Copy to Clipboard
					</Button>
					<Button
						kind="outlined"
						color="neutral"
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
				GitButler will use the system's git credentials helper.
				<Link target="_blank" rel="noreferrer" href="https://git-scm.com/doc/credential-helpers">
					Learn more
				</Link>
			</svelte:fragment>

			<svelte:fragment slot="actions">
				<RadioButton name="credentialType" value="gitCredentialsHelper" id="credential-helper" />
			</svelte:fragment>
		</SectionCard>

		<SectionCard roundedTop={false} orientation="row">
			<CredentialCheck
				bind:this={credentialCheck}
				projectId={project.id}
				{authService}
				remoteName={remoteName || $baseBranch?.remoteName}
				branchName={branchName || $baseBranch?.shortName}
			/>
		</SectionCard>
	</form>
</div>

<style lang="postcss">
	.git-auth-wrap {
		display: flex;
		flex-direction: column;
		gap: var(--space-16);

		& p {
			color: var(--clr-theme-scale-ntrl-30);
		}
	}

	.inputs-group {
		display: flex;
		flex-direction: column;
		gap: var(--space-16);
		width: 100%;
	}

	.input-with-button {
		display: flex;
		gap: var(--space-8);
		align-items: flex-end;
	}

	.row-buttons {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-8);
	}

	.git-radio {
		display: flex;
		flex-direction: column;
	}
</style>
