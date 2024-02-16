<script lang="ts">
	import { deleteAllData } from '$lib/backend/data';
	import AnalyticsSettings from '$lib/components/AnalyticsSettings.svelte';
	import Button from '$lib/components/Button.svelte';
	import GithubIntegration from '$lib/components/GithubIntegration.svelte';
	import IconButton from '$lib/components/IconButton.svelte';
	import Link from '$lib/components/Link.svelte';
	import Login from '$lib/components/Login.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import ScrollableContainer from '$lib/components/ScrollableContainer.svelte';
	import Spacer from '$lib/components/Spacer.svelte';
	import TextBox from '$lib/components/TextBox.svelte';
	import ThemeSelector from '$lib/components/ThemeSelector.svelte';
	import Toggle from '$lib/components/Toggle.svelte';
	import { copyToClipboard } from '$lib/utils/clipboard';
	import * as toasts from '$lib/utils/toasts';
	import { invoke } from '@tauri-apps/api/tauri';
	import type { PageData } from './$types';
	import { goto } from '$app/navigation';

	export let data: PageData;
	const { cloud, user$, userService } = data;

	$: saving = false;

	$: userPicture = $user$?.picture;

	const fileTypes = ['image/jpeg', 'image/png'];

	const validFileType = (file: File) => {
		return fileTypes.includes(file.type);
	};

	const onPictureChange = (e: Event) => {
		const target = e.target as HTMLInputElement;
		const file = target.files?.[0];

		if (file && validFileType(file)) {
			userPicture = URL.createObjectURL(file);
		} else {
			userPicture = $user$?.picture;
			toasts.error('Please use a valid image file');
		}
	};

	let newName = '';

	let loaded = false;
	$: if ($user$ && !loaded) {
		loaded = true;
		cloud.user.get($user$?.access_token).then((cloudUser) => {
			cloudUser.github_access_token = $user$?.github_access_token; // prevent overwriting with null
			userService.setUser(cloudUser);
		});
		newName = $user$?.name || '';
	}

	const onSubmit = async (e: SubmitEvent) => {
		if (!$user$) return;
		saving = true;

		const target = e.target as HTMLFormElement;
		const formData = new FormData(target);
		const picture = formData.get('picture') as File | undefined;

		try {
			const updatedUser = await cloud.user.update($user$.access_token, {
				name: newName,
				picture: picture
			});
			userService.setUser(updatedUser);
			toasts.success('Profile updated');
		} catch (e) {
			console.error(e);
			toasts.error('Failed to update user');
		}
		saving = false;
	};

	let isDeleting = false;
	let deleteConfirmationModal: Modal;

	export function git_get_config(params: { key: string }) {
		return invoke<string>('git_get_global_config', params);
	}

	export function git_set_config(params: { key: string; value: string }) {
		return invoke<string>('git_set_global_config', params);
	}

	const setCommitterSetting = (value: boolean) => {
		annotateCommits = value;
		git_set_config({
			key: 'gitbutler.gitbutlerCommitter',
			value: value ? '1' : '0'
		});
	};

	const setSigningSetting = (value: boolean) => {
		signCommits = value;
		git_set_config({
			key: 'gitbutler.signCommits',
			value: value ? 'true' : 'false'
		});
	};

	export function get_public_key() {
		return invoke<string>('get_public_key');
	}

	let sshKey = '';
	get_public_key().then((key) => {
		sshKey = key;
	});

	$: annotateCommits = true;
	$: signCommits = false;

	git_get_config({ key: 'gitbutler.gitbutlerCommitter' }).then((value) => {
		annotateCommits = value ? value === '1' : false;
	});

	git_get_config({ key: 'gitbutler.signCommits' }).then((value) => {
		signCommits = value ? value === 'true' : false;
	});

	const onDeleteClicked = () =>
		Promise.resolve()
			.then(() => (isDeleting = true))
			.then(() => deleteAllData())
			// TODO: Delete user from observable!!!
			.then(() => userService.logout())
			.then(() => toasts.success('All data deleted'))
			.catch((e) => {
				console.error(e);
				toasts.error('Failed to delete project');
			})
			.then(() => deleteConfirmationModal.close())
			.then(() => goto('/', { replaceState: true, invalidateAll: true }))
			.finally(() => (isDeleting = false));
</script>

<ScrollableContainer wide>
	<div class="settings" data-tauri-drag-region>
		<div class="card">
			<div class="card__header text-base-16 font-semibold">
				<span class="card__title">GitButler Settings</span>
				<IconButton
					icon="cross"
					on:click={() => {
						if (history.length > 0) {
							history.back();
						} else {
							goto('/');
						}
					}}
				/>
			</div>
			<div class="card__content">
				<div>
					<h2 class="text-base-16 text-bold">GitButler Cloud</h2>
					<p class="">
						{#if $user$}
							Your online account details on gitbutler.com
						{:else}
							You are not logged into GitButler.
						{/if}
					</p>
				</div>

				{#if $user$}
					<form
						on:submit={onSubmit}
						class="user-form flex flex-row items-start justify-between gap-12 rounded-lg"
					>
						<div id="profile-picture" class="relative flex flex-col items-center gap-2 pt-4">
							{#if $user$.picture}
								<img
									class="h-28 w-28 rounded-full border-zinc-300"
									src={userPicture}
									alt="Your avatar"
								/>
							{/if}

							<label
								title="Edit profile photo"
								for="picture"
								class="font-sm absolute bottom-0 right-0 ml-16 cursor-default rounded-lg border border-zinc-600 bg-zinc-800 px-2 text-center text-zinc-300 hover:bg-zinc-900 hover:text-zinc-50"
							>
								Edit
								<input
									on:change={onPictureChange}
									type="file"
									id="picture"
									name="picture"
									accept={fileTypes.join('')}
									class="input hidden"
								/>
							</label>
						</div>

						<div id="contact-info" class="flex flex-1 flex-wrap">
							<div class="basis-full pr-4">
								<TextBox label="Full name" bind:value={newName} required />
							</div>

							<div class="mt-4 basis-full pr-4">
								<TextBox label="Email" bind:value={$user$.email} readonly />
							</div>
							<div class="mt-4 basis-full pr-4 text-right">
								<Button loading={saving} color="primary">Update profile</Button>
							</div>
						</div>
					</form>
				{:else}
					<Login {userService} />
				{/if}
				<Spacer />
				<div>
					<h2 class="text-base-16 text-bold">Git Stuff</h2>
				</div>
				<div class="flex items-center">
					<div class="flex-grow">
						<p>Credit GitButler as the Committer</p>
						<div class="space-y-2 pr-8 text-sm text-light-700 dark:text-dark-200">
							<div>
								By default, everything in the GitButler client is free to use, but we credit
								ourselves as the committer in your virtual branch commits.
							</div>
							<Link
								target="_blank"
								rel="noreferrer"
								href="https://docs.gitbutler.com/features/virtual-branches/committer-mark"
							>
								Learn more
							</Link>
						</div>
					</div>
					<div>
						<Toggle
							checked={annotateCommits}
							on:change={() => setCommitterSetting(!annotateCommits)}
						/>
					</div>
				</div>

				<div class="flex flex-col space-y-2">
					<p>SSH Key</p>
					<div class="pr-8 text-sm text-light-700 dark:text-dark-200">
						<div>
							GitButler uses SSH keys to authenticate with your Git provider. Add the following
							public key to your Git provider to enable GitButler to push code.
						</div>
					</div>
					<div class="flex-auto overflow-y-scroll">
						<TextBox readonly selectall bind:value={sshKey} />
					</div>
					<div class="flex flex-row justify-end space-x-2">
						<div>
							<Button kind="filled" color="primary" on:click={() => copyToClipboard(sshKey)}>
								Copy to Clipboard
							</Button>
						</div>
						<div class="p-1">
							<Link target="_blank" rel="noreferrer" href="https://github.com/settings/ssh/new"
								>Add key to GitHub</Link
							>
						</div>
					</div>
				</div>

				<div class="flex items-center">
					<div class="flex-grow">
						<p>Sign Commits with the above SSH Key</p>
						<div class="space-y-2 pr-8 text-sm text-light-700 dark:text-dark-200">
							<div>
								If you want GitButler to sign your commits with the SSH key we generated, then you
								can add that key to GitHub as a signing key to have those commits verified.
							</div>
							<Link
								target="_blank"
								rel="noreferrer"
								href="https://docs.gitbutler.com/features/virtual-branches/verifying-commits"
							>
								Learn more
							</Link>
						</div>
					</div>
					<div>
						<Toggle checked={signCommits} on:change={() => setSigningSetting(!signCommits)} />
					</div>
				</div>

				<Spacer />
				<div>
					<h2 class="text-base-16 text-bold">Appearance</h2>
				</div>
				<div class="flex items-center">
					<div class="flex-grow">
						<p>Interface theme</p>
						<p class="text-sm text-light-700 dark:text-dark-200">
							Select or customize your interface theme.
						</p>
					</div>
					<div><ThemeSelector /></div>
				</div>

				<Spacer />
				<AnalyticsSettings showTitle />
				<Spacer />

				{#if $user$}
					<div>
						<h2 class="text-base-16 text-bold">Remote Integrations</h2>
					</div>
					<GithubIntegration {userService} />
				{/if}

				<div>
					<h2 class="text-base-16 text-bold">Need help?</h2>
				</div>
				<div class="flex gap-x-4">
					<a
						href="https://discord.gg/MmFkmaJ42D"
						target="_blank"
						rel="noreferrer"
						class="flex-1 rounded border border-light-200 bg-white p-4 dark:border-dark-400 dark:bg-dark-700"
					>
						<p class="mb-2 font-medium">Join our Discord</p>
						<p class="text-light-700 dark:text-dark-200">
							Join our community and share feedback, requests, or ask a question.
						</p>
					</a>
					<a
						href="mailto:hello@gitbutler.com?subject=Feedback or question!"
						target="_blank"
						class="flex-1 rounded border border-light-200 bg-white p-4 dark:border-dark-400 dark:bg-dark-700"
					>
						<p class="mb-2 font-medium">Contact us</p>
						<p class="text-light-700 dark:text-dark-200">
							If you have an issue or any questions, contact us.
						</p>
					</a>
				</div>
			</div>
			<div class="card__footer">
				<Button color="error" kind="outlined" on:click={() => deleteConfirmationModal.show()}>
					Delete all data
				</Button>
				{#if $user$}
					<!-- TODO: Separate logout from login button -->
					<Login {userService} />
				{/if}
			</div>
		</div>

		<Modal bind:this={deleteConfirmationModal} title="Delete all local data?">
			<p>Are you sure you want to delete all local data? This can’t be undone.</p>

			<svelte:fragment slot="controls" let:close>
				<Button kind="outlined" on:click={close}>Cancel</Button>
				<Button color="error" loading={isDeleting} on:click={onDeleteClicked}>Delete</Button>
			</svelte:fragment>
		</Modal>
	</div>
</ScrollableContainer>

<div id="clipboard" />

<style lang="postcss">
	.settings {
		display: flex;
		width: 100%;
		max-width: 50rem;
		margin: 0 auto;
		justify-self: center;
		justify-content: center;
		flex-direction: column;
		padding: var(--space-40) var(--space-40);
	}

	.card__content {
		gap: var(--space-24);
	}
	.card__footer {
		justify-content: right;
	}
</style>
