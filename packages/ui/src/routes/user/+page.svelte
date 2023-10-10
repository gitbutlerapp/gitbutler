<script lang="ts">
	import { Button, Modal, Login, Link } from '$lib/components';
	import type { PageData } from './$types';
	import * as toasts from '$lib/toasts';
	import { initDeviceOauth, checkAuthStatus } from '$lib/api/ipc/github';
	import { deleteAllData } from '$lib/api/ipc';
	import { userStore } from '$lib/stores/user';
	import { goto } from '$app/navigation';
	import ThemeSelector from '../ThemeSelector.svelte';
	import { getContext } from 'svelte';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/userSettings';
	import { invoke } from '@tauri-apps/api/tauri';
	import { copyToClipboard } from '$lib/clipboard';

	export let data: PageData;
	const { cloud } = data;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);
	const user = userStore;

	$: saving = false;

	$: userPicture = $user?.picture;

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
			userPicture = $user?.picture;
			toasts.error('Please use a valid image file');
		}
	};

	let newName = '';

	let loaded = false;
	$: if ($user && !loaded) {
		loaded = true;
		cloud.user.get($user?.access_token).then((cloudUser) => {
			cloudUser.github_access_token = $user?.github_access_token; // prevent overwriting with null
			$user = cloudUser;
		});
		newName = $user?.name || '';
	}

	const onSubmit = async (e: SubmitEvent) => {
		if (!$user) return;
		saving = true;

		const target = e.target as HTMLFormElement;
		const formData = new FormData(target);
		const picture = formData.get('picture') as File | undefined;

		try {
			$user = await cloud.user.update($user.access_token, {
				name: newName,
				picture: picture
			});
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
			key: 'gitbutler.utmostDiscretion',
			value: value ? '0' : '1'
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

	git_get_config({ key: 'gitbutler.utmostDiscretion' }).then((value) => {
		annotateCommits = value ? value === '0' : true;
	});

	git_get_config({ key: 'gitbutler.signCommits' }).then((value) => {
		signCommits = value ? value === 'true' : true;
	});

	const onDeleteClicked = () =>
		Promise.resolve()
			.then(() => (isDeleting = true))
			.then(() => deleteAllData())
			.then(() => user.set(null))
			.then(() => toasts.success('All data deleted'))
			.catch((e) => {
				console.error(e);
				toasts.error('Failed to delete project');
			})
			.then(() => deleteConfirmationModal.close())
			.then(() => goto('/', { replaceState: true, invalidateAll: true }))
			.finally(() => (isDeleting = false));
	let userCode = '';
	let deviceCode = '';
	function gitHubStartOauth() {
		initDeviceOauth().then((verification) => {
			userCode = verification.user_code;
			deviceCode = verification.device_code;
			gitHubOauthModal.show();
		});
	}
	let gitHubOauthModal: Modal;
	function gitHubOauthCheckStatus(deviceCode: string) {
		checkAuthStatus({ deviceCode }).then((access_token) => {
			let u = $user;
			if (u) {
				u.github_access_token = access_token;
				$user = u;
			}
		});
	}
</script>

<div class="mx-auto h-fit w-full max-w-xl py-10">
	<div class="flex flex-col gap-y-8">
		<div class="flex w-full justify-between">
			<h2 class="text-2xl font-medium">GitButler Settings</h2>
			{#if $user}
				<!-- TODO: Separate logout from login button -->
				<Login />
			{/if}
		</div>
		<div class="bg-light-400 dark:bg-dark-700 h-[0.0625rem]" />

		<div>
			<h2 class="mb-2 text-lg font-medium">GitButler Cloud</h2>
			<p class="">
				{#if $user}
					Your online account details on gitbutler.com
				{:else}
					You are not logged into GitButler.
				{/if}
			</p>
		</div>

		{#if $user}
			<form
				on:submit={onSubmit}
				class="user-form flex flex-row items-start justify-between gap-12 rounded-lg"
			>
				<div id="profile-picture" class="relative flex flex-col items-center gap-2 pt-4">
					{#if $user.picture}
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
						<label for="fullName" class="text-zinc-400">Full name</label>
						<input
							autocomplete="off"
							autocorrect="off"
							spellcheck="false"
							name="firstName"
							bind:value={newName}
							type="text"
							class="input w-full"
							placeholder="Name can't be empty"
							required
						/>
					</div>

					<div class="mt-4 basis-full pr-4">
						<label for="email" class="text-zinc-400">Email</label>
						<input
							autocomplete="off"
							autocorrect="off"
							spellcheck="false"
							readonly
							id="email"
							name="email"
							bind:value={$user.email}
							type="text"
							class="input w-full"
						/>
					</div>
					<div class="mt-4 basis-full pr-4 text-right">
						<Button loading={saving} color="purple" type="submit">Update profile</Button>
					</div>
				</div>
			</form>
		{:else}
			<Login />
		{/if}
		<div class="bg-light-400 dark:bg-dark-700 h-[0.0625rem]" />
		<div>
			<h2 class="mb-2 text-lg font-medium">Git Stuff</h2>
		</div>
		<div class="flex items-center">
			<div class="flex-grow">
				<p>Credit GitButler as the Committer</p>
				<div class="space-y-2 pr-8 text-sm text-light-700 dark:text-dark-200">
					<div>
						By default, everything in the GitButler client is free to use, but we credit ourselves
						as the committer in your virtual branch commits. Community members and supporters of
						GitButler can turn this off.
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
				<label class="relative inline-flex cursor-pointer items-center">
					<input
						type="checkbox"
						disabled={!$user?.supporter}
						checked={annotateCommits}
						on:change={(e) => setCommitterSetting(!!e.currentTarget?.checked)}
						class="peer sr-only"
					/>
					<div
						class="peer h-6 w-11 rounded-full bg-gray-400 after:absolute after:left-[2px] after:top-[2px] after:h-5 after:w-5 after:rounded-full after:bg-white after:transition-all after:content-[''] peer-checked:bg-purple-600 peer-checked:after:translate-x-full peer-checked:after:border-white peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 peer-disabled:bg-zinc-300 dark:border-gray-600 dark:bg-gray-700 dark:peer-focus:ring-blue-800"
					/>
				</label>
			</div>
		</div>

		<div class="flex flex-col space-y-2">
			<p>SSH Key</p>
			<div class="text-light-700 dark:text-dark-200 pr-8 text-sm">
				<div>
					GitButler uses SSH keys to authenticate with your Git provider. Add the following public
					key to your Git provider to enable GitButler to push code.
				</div>
			</div>
			<div class="flex-auto overflow-y-scroll">
				<input
					bind:value={sshKey}
					class="
                        whitespece-pre border-light-200 dark:border-dark-400 dark:bg-dark-700 w-full select-all rounded border bg-white p-2 font-mono"
				/>
			</div>
			<div class="flex flex-row justify-end space-x-2">
				<div>
					<Button kind="filled" color="purple" on:click={() => copyToClipboard(sshKey)}>
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
				<div class="text-light-700 dark:text-dark-200 space-y-2 pr-8 text-sm">
					<div>
						If you want GitButler to sign your commits with the SSH key we generated, then you can
						add that key to GitHub as a signing key to have those commits verified.
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
				<label class="relative inline-flex cursor-pointer items-center">
					<input
						type="checkbox"
						checked={signCommits}
						on:change={(e) => setSigningSetting(!!e.currentTarget?.checked)}
						class="peer sr-only"
					/>
					<div
						class="peer h-6 w-11 rounded-full bg-gray-400 after:absolute after:left-[2px] after:top-[2px] after:h-5 after:w-5 after:rounded-full after:bg-white after:transition-all after:content-[''] peer-checked:bg-purple-600 peer-checked:after:translate-x-full peer-checked:after:border-white peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 peer-disabled:bg-zinc-300 dark:border-gray-600 dark:bg-gray-700 dark:peer-focus:ring-blue-800"
					/>
				</label>
			</div>
		</div>

		<div class="bg-light-400 dark:bg-dark-700 h-[0.0625rem]" />
		<div>
			<h2 class="mb-2 text-lg font-medium">Appearance</h2>
		</div>
		<div class="flex items-center">
			<div class="flex-grow">
				<p>Generate descriptions for code changes</p>
				<p class="text-light-700 dark:text-dark-200 text-sm">
					GitButler Cloud will generate descriptions for code hunks in your virtual branches board.
				</p>
			</div>
			<div>
				<label class="relative inline-flex cursor-pointer items-center">
					<input
						type="checkbox"
						checked={$userSettings.aiSummariesEnabled}
						on:change={(e) =>
							userSettings.update((s) => ({
								...s,
								aiSummariesEnabled: !!e.currentTarget?.checked
							}))}
						class="peer sr-only"
					/>
					<div
						class="peer h-6 w-11 rounded-full bg-gray-400 after:absolute after:left-[2px] after:top-[2px] after:h-5 after:w-5 after:rounded-full after:bg-white after:transition-all after:content-[''] peer-checked:bg-purple-600 peer-checked:after:translate-x-full peer-checked:after:border-white peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:border-gray-600 dark:bg-gray-700 dark:peer-focus:ring-blue-800"
					/>
				</label>
			</div>
		</div>
		<div class="flex items-center">
			<div class="flex-grow">
				<p>Interface theme</p>
				<p class="text-light-700 dark:text-dark-200 text-sm">
					Select or customize your interface theme.
				</p>
			</div>
			<div><ThemeSelector /></div>
		</div>

		<div class="h-[0.0625rem] bg-light-400 dark:bg-dark-700" />

		{#if $user}
			<div>
				<h2 class="mb-2 text-lg font-medium">Remote Integrations</h2>
			</div>
			<div class="flex items-center">
				<div class="flex-grow">
					<p>
						GitHub
						{#if $user.github_access_token}
							<span class="text-sm text-green-500">️✅ — already configured</span>
						{/if}
					</p>
					<p class="text-sm text-light-700 dark:text-dark-200">
						Allows you to view and create Pull Requests from GitButler.
					</p>
				</div>
				<div>
					<Button kind="filled" color="purple" on:click={gitHubStartOauth}>
						{#if $user.github_access_token}
							Reauthenticate
						{:else}
							Set up
						{/if}
					</Button>
				</div>
			</div>

			<div class="h-[0.0625rem] bg-light-400 dark:bg-dark-700" />
		{/if}

		<div>
			<h2 class="mb-2 text-lg font-medium">Need help?</h2>
		</div>
		<div class="flex gap-x-4">
			<a
				href="https://discord.gg/wDKZCPEjXC"
				target="_blank"
				rel="noreferrer"
				class="border-light-200 dark:border-dark-400 dark:bg-dark-700 flex-1 rounded border bg-white p-4"
			>
				<p class="mb-2 font-medium">Join our Discorder</p>
				<p class="text-light-700 dark:text-dark-200">
					Join our community and share feedback, requests, or ask a question.
				</p>
			</a>
			<a
				href="mailto:hello@gitbutler.com?subject=Feedback or question!"
				target="_blank"
				class="border-light-200 dark:border-dark-400 dark:bg-dark-700 flex-1 rounded border bg-white p-4"
			>
				<p class="mb-2 font-medium">Contact us</p>
				<p class="text-light-700 dark:text-dark-200">
					If you have an issue or any questions, contact us.
				</p>
			</a>
		</div>

		<div class="bg-light-400 dark:bg-dark-700 h-[0.0625rem]" />

		<div class="flex flex-col gap-4">
			<Button color="destructive" kind="outlined" on:click={() => deleteConfirmationModal.show()}>
				Delete all data
			</Button>
		</div>

		<Modal bind:this={deleteConfirmationModal} title="Delete all local data?">
			<p>Are you sure you want to delete all local data? This can’t be undone.</p>

			<svelte:fragment slot="controls" let:close>
				<Button kind="outlined" on:click={close}>Cancel</Button>
				<Button color="destructive" loading={isDeleting} on:click={onDeleteClicked}>Delete</Button>
			</svelte:fragment>
		</Modal>

		<Modal on:close={() => gitHubOauthCheckStatus(deviceCode)} bind:this={gitHubOauthModal} title="Authenticate with GitHub">
			<div class="flex flex-col gap-4">
				<div class="flex items-center gap-2">
					<span class="flex-grow">1️⃣ Copy the following verification code: </span>
					<input
						bind:value={userCode}
						class="
						whitespece-pre h-6 w-24 select-all rounded border border-light-200 bg-white font-mono dark:border-dark-400 dark:bg-dark-700"
					/>

					<Button kind="outlined" color="purple" on:click={() => copyToClipboard(userCode)}>
						Copy to Clipboard
					</Button>
				</div>
				<div>
					2️⃣ Navigate to
					<a class="underline" href="https://github.com/login/device" target="_blank" rel="noreferrer"
						>https://github.com/login/device</a
					>
				</div>
				<div>3️⃣ Paste the code that you copied and follow the on-screen instructions.</div>
			</div>
			<svelte:fragment slot="controls" let:close>
				<Button color="purple" on:click={close}
					>Done</Button
				>
			</svelte:fragment>
		</Modal>
	</div>
</div>

<div id="clipboard" />
