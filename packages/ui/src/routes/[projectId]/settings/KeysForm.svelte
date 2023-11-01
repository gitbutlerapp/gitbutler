<script lang="ts">
	import type { Key, Project } from '$lib/api/ipc/projects';
	import { Button, Link } from '$lib/components';
	import { invoke } from '$lib/ipc';
	import { copyToClipboard } from '$lib/clipboard';
	import { createEventDispatcher } from 'svelte';

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

	let selectedOption = project.preferred_key === 'generated' ? 'generated' : 'local';

	let privateKeyPath =
		project.preferred_key === 'generated' ? '' : project.preferred_key.local.private_key_path;
	let privateKeyPassphrase =
		project.preferred_key === 'generated' ? '' : project.preferred_key.local.passphrase;
	function setLocalKey() {
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

	function setGeneratedKey() {
		dispatch('updated', {
			preferred_key: 'generated'
		});
	}
</script>

<div class="flex flex-col gap-1">
	<p>Preferred SSH Key</p>
	<div class="pr-8 text-sm text-light-700 dark:text-dark-200">
		<div>
			Select the SSH key that GitButler will use to authenticate with your Git provider. These keys
			are unique for your every GitButler client and are never sent anywhere.
		</div>
	</div>

	<div class="grid grid-cols-2 gap-2" style="grid-template-columns: max-content 1fr;">
		<input type="radio" bind:group={selectedOption} value="generated" on:input={setGeneratedKey} />

		<div class="flex flex-col space-y-2">
			<div class="pr-8">
				<div>Use locally generated SSH key</div>
			</div>
			{#if selectedOption === 'generated'}
				<div>
					Add the following public key to your Git provider to enable GitButler to push code.
				</div>
				<div class="flex-auto overflow-y-scroll">
					<input
						bind:value={sshKey}
						disabled={selectedOption !== 'generated'}
						class="whitespece-pre w-full select-all rounded border border-light-200 bg-white p-2 font-mono dark:border-dark-400 dark:bg-dark-700"
					/>
				</div>
				<div class="flex flex-row justify-end space-x-2">
					<div>
						<Button
							kind="filled"
							color="purple"
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

		<input type="radio" bind:group={selectedOption} value="local" on:input={setLocalKey} />
		<div class="flex flex-col space-y-2">
			<div>Use existing SSH key</div>

			{#if selectedOption === 'local'}
				<div class="pr-8">
					<div>
						Select the SSH key that GitButler will use to authenticate with your Git provider.
					</div>
				</div>

				<div class="grid grid-cols-2 gap-2" style="grid-template-columns: max-content 1fr;">
					<label for="path">Path to private key</label>
					<input
						name="path"
						type="text"
						placeholder="~/.ssh/id_rsa"
						bind:value={privateKeyPath}
						on:input={setLocalKey}
					/>

					<label for="passphrase">Passphrase</label>
					<input
						name="passphrase"
						type="password"
						bind:value={privateKeyPassphrase}
						on:input={setLocalKey}
					/>
				</div>
			{/if}
		</div>
	</div>
</div>
