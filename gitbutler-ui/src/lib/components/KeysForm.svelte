<script lang="ts">
	import { invoke } from '$lib/backend/ipc';
	import Button from '$lib/components/Button.svelte';
	import Link from '$lib/components/Link.svelte';
	import TextBox from '$lib/components/TextBox.svelte';
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
</script>

<div class="flex flex-col gap-1">
	<p>Git Authentication</p>
	<div class="pr-8 text-sm text-light-700 dark:text-dark-200">
		<div>
			Configure the authentication flow for GitButler when authenticating with your Git remote
			provider.
		</div>
	</div>

	<div class="grid grid-cols-2 gap-2" style="grid-template-columns: max-content 1fr;">
		<input type="radio" bind:group={selectedOption} value="default" on:input={setDefaultKey} />
		<div class="flex flex-col space-y-2">
			<div>Auto detect</div>
			{#if selectedOption === 'default'}
				<div class="pr-8 text-sm text-light-700 dark:text-dark-200">
					<div>GitButler will attempt all available authentication flows automatically.</div>
				</div>
			{/if}
		</div>

		<input type="radio" bind:group={selectedOption} value="local" />
		<div class="flex flex-col space-y-2">
			<div>Use existing SSH key</div>

			{#if selectedOption === 'local'}
				<div class="pr-8 text-sm text-light-700 dark:text-dark-200">
					Add the path to an existing SSH key that GitButler can use.
				</div>

				<div
					class="grid grid-cols-2 items-center gap-2"
					style="grid-template-columns: max-content 1fr;"
				>
					<label for="path">Path to private key</label>

					<TextBox
						placeholder="for example: ~/.ssh/id_rsa"
						bind:value={privateKeyPath}
						on:input={debounce(setLocalKey, 600)}
					/>

					<label for="passphrase">Passphrase (optional)</label>
					<TextBox
						type="password"
						bind:value={privateKeyPassphrase}
						on:input={debounce(setLocalKey, 600)}
					/>
				</div>
			{/if}
		</div>

		<input type="radio" bind:group={selectedOption} value="generated" on:input={setGeneratedKey} />
		<div class="flex flex-col space-y-2">
			<div class="pr-8">
				<div>Use locally generated SSH key</div>
			</div>
			{#if selectedOption === 'generated'}
				<div class="pr-8 text-sm text-light-700 dark:text-dark-200">
					GitButler will use a locally generated SSH key. For this to work you <b>need</b>
					to add the following public key to your Git remote provider:
				</div>
				<div class="flex-auto overflow-y-scroll">
					<input
						bind:value={sshKey}
						disabled={selectedOption !== 'generated'}
						class="whitespece-pre input w-full select-all rounded border p-2 font-mono"
					/>
				</div>
				<div class="flex flex-row justify-end space-x-2">
					<div>
						<Button
							kind="filled"
							color="primary"
							on:click={() => copyToClipboard(sshKey)}
							disabled={selectedOption !== 'generated'}
						>
							Copy to Clipboard
						</Button>
					</div>
					<div class="p-1">
						<Link
							target="_blank"
							rel="noreferrer"
							href="https://github.com/settings/ssh/new"
							disabled={selectedOption !== 'generated'}
						>
							Add key to GitHub
						</Link>
					</div>
				</div>
			{/if}
		</div>

		<input
			type="radio"
			bind:group={selectedOption}
			value="gitCredentialsHelper"
			on:input={setGitCredentialsHelperKey}
		/>
		<div class="flex flex-col space-y-2">
			<div class="pr-8">
				<div>
					Use a
					<Link target="_blank" rel="noreferrer" href="https://git-scm.com/doc/credential-helpers"
						>Git credentials helper</Link
					>
				</div>
			</div>
			{#if selectedOption === 'gitCredentialsHelper'}
				<div class="pr-8 text-sm text-light-700 dark:text-dark-200">
					GitButler will use the system's git credentials helper.
				</div>
			{/if}
		</div>
	</div>
</div>
