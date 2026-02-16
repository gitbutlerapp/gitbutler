<script lang="ts">
	import AddSshKeyModal from '$lib/components/AddSshKeyModal.svelte';
	import { CardGroup, Spacer } from '@gitbutler/ui';
	import type { SshKey, SshKeyService } from '$lib/sshKeyService';
	import type { UserService } from '$lib/user/userService';

	interface Props {
		sshKeyService: SshKeyService;
		userService: UserService;
	}

	let { sshKeyService, userService }: Props = $props();

	let sshKeys = $state<SshKey[]>([]);
	let loadingSshKeys = $state(true);
	let generatingSshToken = $state(false);
	let sshKeyToken = $state('');
	let showSshKeyTokenModal = $state(false);
	let addKeyModal = $state<AddSshKeyModal>();

	$effect(() => {
		loadSshKeys();
	});

	async function loadSshKeys() {
		try {
			sshKeys = await sshKeyService.getSshKeys();
		} catch (error) {
			console.error('Failed to load SSH keys:', error);
		} finally {
			loadingSshKeys = false;
		}
	}

	async function deleteSshKey(fingerprint: string) {
		const key = sshKeys.find((k) => k.fingerprint === fingerprint);
		if (!key) return;

		const confirmed = confirm(`Are you sure you want to delete the SSH key "${key.name}"?`);
		if (!confirmed) return;

		try {
			await sshKeyService.deleteSshKey(key.fingerprint);
			sshKeys = sshKeys.filter((key) => key.fingerprint !== fingerprint);
		} catch (error) {
			console.error('Failed to delete SSH key:', error);
		}
	}

	async function onAddKeyModalClose() {
		await loadSshKeys();
	}

	async function generateSshKeyToken() {
		generatingSshToken = true;
		try {
			const updatedUser = await userService.updateUser({
				generate_ssh_token: true
			});

			if (updatedUser && updatedUser.ssh_key_token) {
				sshKeyToken = updatedUser.ssh_key_token;
				showSshKeyTokenModal = true;
			} else {
				console.error('Failed to generate SSH key token: No token returned');
			}
		} catch (error) {
			console.error('Failed to generate SSH key token:', error);
		} finally {
			generatingSshToken = false;
		}
	}

	function closeSshKeyTokenModal() {
		showSshKeyTokenModal = false;
		sshKeyToken = '';
	}

	function handleModalClick(e: Event) {
		e.stopPropagation();
	}
</script>

<Spacer />

<CardGroup.Item standalone>
	<div class="ssh-keys">
		{#if loadingSshKeys}
			<div class="loading">Loading SSH keys...</div>
		{:else if sshKeys.length === 0}
			<div class="no-keys">No SSH keys added yet</div>
		{:else}
			{#each sshKeys as key}
				<div class="ssh-key">
					<div class="ssh-key-info">
						<span class="ssh-key-name">{key.name}</span>
						<span class="ssh-key-fingerprint">{key.fingerprint}</span>
					</div>
					<button
						type="button"
						class="delete-button"
						title="Delete key"
						onclick={() => deleteSshKey(key.fingerprint)}>×</button
					>
				</div>
			{/each}
		{/if}

		<button type="button" class="add-key-button" onclick={() => addKeyModal?.show()}>
			<span class="add-key-icon">+</span>
			<span>Upload SSH Public Key</span>
		</button>

		<button
			type="button"
			class="add-key-button"
			onclick={generateSshKeyToken}
			disabled={generatingSshToken}
		>
			<span class="add-key-icon">+</span>
			<span>{generatingSshToken ? 'Generating...' : 'Add Key via SSH'}</span>
		</button>
	</div>
</CardGroup.Item>

<AddSshKeyModal bind:this={addKeyModal} onClose={onAddKeyModalClose} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
{#if showSshKeyTokenModal}
	<div class="ssh-token-modal-backdrop" onclick={closeSshKeyTokenModal}>
		<div class="ssh-token-modal" onclick={handleModalClick}>
			<h3 class="ssh-token-modal__title">Add Your SSH Key</h3>
			<p class="ssh-token-modal__description">
				Run the following command in your terminal to add your SSH key to GitButler:
			</p>
			<div class="ssh-token-modal__code">
				<code>ssh git@ssh.gitbutler.com add/{sshKeyToken}</code>
				<button
					type="button"
					class="ssh-token-modal__copy-button"
					onclick={() => {
						navigator.clipboard.writeText(`ssh git@ssh.gitbutler.com add/${sshKeyToken}`);
					}}
				>
					Copy
				</button>
			</div>
			<p class="ssh-token-modal__note">This token will expire after use or in 5 minutes.</p>
			<div class="ssh-token-modal__controls">
				<button type="button" class="ssh-token-modal__close-button" onclick={closeSshKeyTokenModal}
					>Close</button
				>
			</div>
		</div>
	</div>
{/if}

<style lang="postcss">
	.ssh-keys {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.ssh-key {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
	}

	.ssh-key-info {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.ssh-key-name {
		color: var(--clr-text-1);
		font-weight: 500;
		font-size: 14px;
	}

	.ssh-key-fingerprint {
		color: var(--clr-text-2);
		font-size: 13px;
		font-family: var(--font-mono);
	}

	.delete-button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-s);
		background-color: transparent;
		color: var(--clr-text-2);
		font-size: 18px;
		line-height: 1;
		cursor: pointer;
		transition: all var(--transition-medium);

		&:hover {
			border-color: var(--clr-text-2);
			background-color: var(--clr-bg-2);
			color: var(--clr-text-2);
		}
	}

	.add-key-button {
		display: flex;
		align-items: center;
		margin-bottom: 8px;
		padding: 12px;
		gap: 8px;
		border: 1px dashed var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: transparent;
		color: var(--clr-text-2);
		font-size: 14px;
		cursor: pointer;
		transition: all var(--transition-medium);

		&:hover:not(:disabled) {
			border-color: var(--clr-theme-pop-element);
			color: var(--clr-theme-pop-element);
		}

		&:disabled {
			cursor: not-allowed;
			opacity: 0.5;
		}
	}

	.add-key-icon {
		font-size: 18px;
		line-height: 1;
	}

	.loading,
	.no-keys {
		padding: 24px;
		color: var(--clr-text-2);
		font-size: 14px;
		text-align: center;
	}

	.ssh-token-modal-backdrop {
		display: flex;
		z-index: 1000;
		position: fixed;
		top: 0;
		right: 0;
		bottom: 0;
		left: 0;
		align-items: center;
		justify-content: center;
		background-color: rgba(0, 0, 0, 0.5);
	}

	.ssh-token-modal {
		display: flex;
		flex-direction: column;
		width: 600px;
		max-width: 90vw;
		padding: 24px;
		gap: 16px;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
	}

	.ssh-token-modal__title {
		color: var(--clr-text-1);
		font-weight: 600;
		font-size: 18px;
	}

	.ssh-token-modal__description {
		color: var(--clr-text-2);
		font-size: 14px;
		line-height: 1.5;
	}

	.ssh-token-modal__code {
		position: relative;
		padding: 16px;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
		color: var(--clr-text-1);
		font-size: 13px;
		line-height: 1.4;
		font-family: var(--font-mono);
		word-break: break-all;
	}

	.ssh-token-modal__copy-button {
		position: absolute;
		top: 8px;
		right: 8px;
		padding: 4px 8px;
		border: none;
		border-radius: var(--radius-s);
		background-color: var(--clr-text-1);
		color: var(--clr-bg-1);
		font-size: 12px;
		cursor: pointer;
		transition: background-color var(--transition-medium);

		&:hover {
			background-color: var(--clr-text-2);
		}
	}

	.ssh-token-modal__note {
		color: var(--clr-text-2);
		font-size: 13px;
		line-height: 1.5;
	}

	.ssh-token-modal__controls {
		display: flex;
		justify-content: flex-end;
		margin-top: 8px;
	}

	.ssh-token-modal__close-button {
		padding: 8px 16px;
		border: none;
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-pop-element);
		color: var(--clr-theme-pop-on-element);
		font-size: 14px;
		cursor: pointer;
		transition: background-color var(--transition-medium);

		&:hover {
			background-color: var(--clr-theme-pop-element);
			opacity: 0.9;
		}
	}
</style>
