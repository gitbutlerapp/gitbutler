<script lang="ts">
	import ClickableCard from './ClickableCard.svelte';
	import RadioButton from './RadioButton.svelte';
	import SectionCard from './SectionCard.svelte';
	import Spacer from './Spacer.svelte';
	import TextBox from './TextBox.svelte';
	import { invoke } from '$lib/backend/ipc';
	import Button from '$lib/components/Button.svelte';
	import Link from '$lib/components/Link.svelte';
	import { copyToClipboard } from '$lib/utils/clipboard';
	import { debounce } from '$lib/utils/debounce';
	import { createEventDispatcher } from 'svelte';
	import type { Key, Project } from '$lib/backend/projects';

	export let project: Project;

	const dispatch = createEventDispatcher<{
		updated: {
			preferred_key: Key;
		};
	}>();

	export function get_public_key() {
		return invoke<string>('get_public_key');
	}

	let sshKey = '';
	get_public_key().then((key) => {
		sshKey = key;
	});

	let selectedOption =
		project.preferred_key === 'generated'
			? 'generated'
			: project.preferred_key === 'default'
				? 'default'
				: project.preferred_key === 'gitCredentialsHelper'
					? 'gitCredentialsHelper'
					: 'local';

	let privateKeyPath =
		project.preferred_key === 'generated' ||
		project.preferred_key === 'default' ||
		project.preferred_key === 'gitCredentialsHelper'
			? ''
			: project.preferred_key.local.private_key_path;

	let privateKeyPassphrase =
		project.preferred_key === 'generated' ||
		project.preferred_key === 'default' ||
		project.preferred_key === 'gitCredentialsHelper'
			? ''
			: project.preferred_key.local.passphrase;

	function setLocalKey() {
		if (privateKeyPath.length) {
			dispatch('updated', {
				preferred_key: {
					local: {
						private_key_path: privateKeyPath,
						passphrase:
							privateKeyPassphrase && privateKeyPassphrase.length > 0
								? privateKeyPassphrase
								: undefined
					}
				}
			});
		}
	}

	function setGitCredentialsHelperKey() {
		dispatch('updated', {
			preferred_key: 'gitCredentialsHelper'
		});
	}

	function setDefaultKey() {
		dispatch('updated', {
			preferred_key: 'default'
		});
	}

	function setGeneratedKey() {
		dispatch('updated', {
			preferred_key: 'generated'
		});
	}

	let showPassphrase = false;
</script>

<section class="git-auth-wrap">
	<h3 class="text-base-15 text-bold">Git Authentication</h3>
	<p class="text-base-body-12">
		Configure the authentication flow for GitButler when authenticating with your Git remote
		provider.
	</p>

	<form>
		<fieldset class="git-radio">
			<ClickableCard
				hasBottomRadius={false}
				on:click={() => {
					if (selectedOption == 'default') return;

					selectedOption = 'default';
					setDefaultKey();
				}}
			>
				<svelte:fragment slot="title">Auto detect</svelte:fragment>

				<svelte:fragment slot="actions">
					<RadioButton bind:group={selectedOption} value="default" on:input={setDefaultKey} />
				</svelte:fragment>

				<svelte:fragment slot="body">
					GitButler will attempt all available authentication flows automatically.
				</svelte:fragment>
			</ClickableCard>

			<ClickableCard
				hasTopRadius={false}
				hasBottomRadius={false}
				hasBottomLine={selectedOption !== 'local'}
				on:click={() => {
					selectedOption = 'local';
				}}
			>
				<svelte:fragment slot="title">Use existing SSH key</svelte:fragment>

				<svelte:fragment slot="actions">
					<RadioButton bind:group={selectedOption} value="local" />
				</svelte:fragment>

				<svelte:fragment slot="body">
					Add the path to an existing SSH key that GitButler can use.
				</svelte:fragment>
			</ClickableCard>

			{#if selectedOption === 'local'}
				<SectionCard hasTopRadius={false} hasBottomRadius={false}>
					<div class="inputs-group">
						<TextBox
							label="Path to private key"
							placeholder="for example: ~/.ssh/id_rsa"
							bind:value={privateKeyPath}
							on:input={debounce(setLocalKey, 600)}
						/>

						<div class="input-with-button">
							<TextBox
								label="Passphrase (optional)"
								type={showPassphrase ? 'text' : 'password'}
								bind:value={privateKeyPassphrase}
								on:input={debounce(setLocalKey, 600)}
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

			<ClickableCard
				hasTopRadius={false}
				hasBottomRadius={false}
				hasBottomLine={selectedOption !== 'generated'}
				on:click={() => {
					if (selectedOption == 'generated') return;

					selectedOption = 'generated';
					setGeneratedKey();
				}}
			>
				<svelte:fragment slot="title">Use locally generated SSH key</svelte:fragment>

				<svelte:fragment slot="actions">
					<RadioButton bind:group={selectedOption} value="generated" />
				</svelte:fragment>

				<svelte:fragment slot="body">
					GitButler will use a locally generated SSH key. For this to work you need to add the
					following public key to your Git remote provider:
				</svelte:fragment>
			</ClickableCard>

			{#if selectedOption === 'generated'}
				<SectionCard hasTopRadius={false} hasBottomRadius={false}>
					<TextBox readonly selectall bind:value={sshKey} />
					<div class="row-buttons">
						<Button
							kind="filled"
							color="primary"
							icon="copy"
							on:click={() => copyToClipboard(sshKey)}
						>
							Copy to Clipboard
						</Button>
						<Button
							kind="outlined"
							color="neutral"
							icon="open-link"
							on:click={() => {
								open('https://github.com/settings/ssh/new');
							}}
						>
							Add key to GitHub
						</Button>
					</div>
				</SectionCard>
			{/if}

			<ClickableCard
				hasTopRadius={false}
				on:click={() => {
					if (selectedOption == 'gitCredentialsHelper') return;

					selectedOption = 'gitCredentialsHelper';
					setGitCredentialsHelperKey();
				}}
			>
				<svelte:fragment slot="title">Use a Git credentials helper</svelte:fragment>

				<svelte:fragment slot="body">
					GitButler will use the system's git credentials helper.
					<Link target="_blank" rel="noreferrer" href="https://git-scm.com/doc/credential-helpers">
						Learn more
					</Link>
				</svelte:fragment>

				<svelte:fragment slot="actions">
					<RadioButton bind:group={selectedOption} value="gitCredentialsHelper" />
				</svelte:fragment>
			</ClickableCard>
		</fieldset>
	</form>
</section>

<Spacer />

<style>
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
