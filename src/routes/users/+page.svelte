<script lang="ts">
	import { Button, Modal, Login } from '$lib/components';
	import type { PageData } from './$types';
	import { log, toasts } from '$lib';
	import { deleteAllData } from '$lib/api';
	import { goto } from '$app/navigation';
	import { open } from '@tauri-apps/api/shell';

	export let data: PageData;
	const { user, cloud } = data;

	$: saving = false;

	let userNameInput = $user?.name;
	let pictureChanged = false;
	user.subscribe((user) => {
		if (user) {
			userNameInput = user.name;
		}
	});
	$: canTriggerUpdate = (userNameInput !== $user?.name || pictureChanged) && userNameInput;

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
			pictureChanged = true;
		} else {
			userPicture = $user?.picture;
			toasts.error('Please use a valid image file');
		}
	};

	const onSubmit = async (e: SubmitEvent) => {
		if (!$user) return;
		saving = true;

		const target = e.target as HTMLFormElement;
		const formData = new FormData(target);
		const picture = formData.get('picture') as File | undefined;

		try {
			$user = await cloud.user.update($user.access_token, {
				name: userNameInput,
				picture: picture
			});
			toasts.success('Profile updated');
		} catch (e) {
			log.error(e);
			toasts.error('Failed to update user');
		}

		userNameInput = $user?.name;
		pictureChanged = false;
		saving = false;
	};

	let isDeleting = false;
	let deleteConfirmationModal: Modal;

	const onDeleteClicked = () =>
		Promise.resolve()
			.then(() => (isDeleting = true))
			.then(() => deleteAllData())
			.then(() => user.delete())
			.then(() => toasts.success('All data deleted'))
			.catch((e) => {
				log.error(e);
				toasts.error('Failed to delete project');
			})
			.then(() => deleteConfirmationModal.close())
			.then(() => goto('/', { replaceState: true, invalidateAll: true }))
			.finally(() => (isDeleting = false));
</script>

<div class="mx-auto max-w-xl p-8">
	{#if $user}
		<div class="flex flex-col gap-6 text-zinc-100">
			<header class="flex items-center justify-between">
				<div class="flex flex-col">
					<h2 class="text-2xl font-medium">GitButler Cloud Account</h2>
					<div class="">Your online account details on gitbutler.com</div>
				</div>
				<Login {user} {cloud} />
			</header>

			<form
				on:submit={onSubmit}
				class="user-form flex flex-row items-start justify-between gap-12 rounded-lg py-2"
			>
				<fields id="right" class="flex flex-col items-center gap-2 pt-6">
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
						class="font-sm -mt-6 -ml-16 cursor-default rounded-lg border border-zinc-600 bg-zinc-800 px-2 text-center text-zinc-300 hover:bg-zinc-900 hover:text-zinc-50"
					>
						Edit
						<input
							on:change={onPictureChange}
							type="file"
							id="picture"
							name="picture"
							accept={fileTypes.join('')}
							class="hidden"
						/>
					</label>
				</fields>

				<fields id="left" class="flex flex-1 flex-col gap-3">
					<div class="flex flex-col gap-1">
						<label for="name" class="text-zinc-400">Name</label>
						<input
							autocomplete="off"
							autocorrect="off"
							spellcheck="false"
							id="name"
							name="name"
							bind:value={userNameInput}
							type="text"
							class="w-full rounded border border-zinc-600 bg-zinc-700 px-4 py-2 text-zinc-300"
							placeholder="Name can't be empty"
							required
						/>
					</div>

					<div class="flex flex-col gap-1">
						<label for="email" class="text-zinc-400">Email</label>
						<input
							autocomplete="off"
							autocorrect="off"
							spellcheck="false"
							disabled
							id="email"
							name="email"
							bind:value={$user.email}
							type="text"
							class="w-full rounded border border-zinc-600 bg-zinc-700 px-4 py-2 text-zinc-300"
						/>
					</div>

					<footer class="flex justify-end pt-4">
						<Button disabled={!canTriggerUpdate} loading={saving} color="primary" type="submit">
							Update profile
						</Button>
					</footer>
				</fields>
			</form>
		</div>
	{:else}
		<div class="flex flex-col items-center justify-items-center space-y-6 text-white">
			<div class="text-4xl font-bold text-white">Connect to GitButler Cloud</div>
			<div>Sign up or log in to GitButler Cloud for more tools and features:</div>
			<ul class="space-y-2 pb-4 text-zinc-400">
				<li class="flex flex-row space-x-3">
					<svg
						xmlns="http://www.w3.org/2000/svg"
						fill="none"
						viewBox="0 0 24 24"
						stroke-width="1.5"
						stroke="white"
						class="h-6 w-6"
					>
						<path
							stroke-linecap="round"
							stroke-linejoin="round"
							d="M12 16.5V9.75m0 0l3 3m-3-3l-3 3M6.75 19.5a4.5 4.5 0 01-1.41-8.775 5.25 5.25 0 0110.233-2.33 3 3 0 013.758 3.848A3.752 3.752 0 0118 19.5H6.75z"
						/>
					</svg>
					<span>Backup everything you do in any of your projects</span>
				</li>
				<li class="flex flex-row space-x-3">
					<svg
						xmlns="http://www.w3.org/2000/svg"
						fill="none"
						viewBox="0 0 24 24"
						stroke-width="1.5"
						stroke="white"
						class="h-6 w-6"
					>
						<path
							stroke-linecap="round"
							stroke-linejoin="round"
							d="M7.5 21L3 16.5m0 0L7.5 12M3 16.5h13.5m0-13.5L21 7.5m0 0L16.5 12M21 7.5H7.5"
						/>
					</svg>

					<span>Sync your data across devices</span>
				</li>
				<li class="flex flex-row space-x-3">
					<svg
						xmlns="http://www.w3.org/2000/svg"
						fill="none"
						viewBox="0 0 24 24"
						stroke-width="1.5"
						stroke="white"
						class="h-6 w-6"
					>
						<path
							stroke-linecap="round"
							stroke-linejoin="round"
							d="M16.862 4.487l1.687-1.688a1.875 1.875 0 112.652 2.652L10.582 16.07a4.5 4.5 0 01-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 011.13-1.897l8.932-8.931zm0 0L19.5 7.125M18 14v4.75A2.25 2.25 0 0115.75 21H5.25A2.25 2.25 0 013 18.75V8.25A2.25 2.25 0 015.25 6H10"
						/>
					</svg>
					<span>AI commit message automated suggestions</span>
				</li>
			</ul>
			<div class="mt-8 text-center">
				<Login {user} {cloud} />
			</div>
			<div class="text-center text-zinc-300">
				You will still need to give us permission for each project before we transfer any data to
				our servers. You can revoke this permission at any time.
			</div>
		</div>
	{/if}

	<div class="mt-8 flex flex-col gap-4 border-t border-zinc-400 pt-4">
		<button
			class="flex flex-col gap-2 rounded border border-zinc-700 bg-card-default p-3 text-zinc-400 shadow transition duration-150 ease-out hover:bg-card-active hover:ease-in"
			on:click={() => open('mailto:hello@gitbutler.com?subject=Feedback or question!')}
		>
			<h2 class="text-lg text-zinc-300">Get Support</h2>
			<div class="text-zinc-500">If you have an issue or any questions, please email us.</div>
		</button>
		<button
			class="flex flex-col gap-2 rounded border border-zinc-700 bg-card-default p-3 text-zinc-400 shadow transition duration-150 ease-out hover:bg-card-active hover:ease-in"
			on:click={() => open('https://discord.gg/wDKZCPEjXC')}
		>
			<h2 class="text-lg text-zinc-300">Join our Discord</h2>
			<div class="text-zinc-500">Share feedback, request, or ask questions</div>
		</button>
	</div>

	<div class="mt-8 flex flex-col gap-4 border-t border-zinc-400 pt-4">
		<Button color="destructive" kind="outlined" on:click={() => deleteConfirmationModal.show()}>
			Delete all data
		</Button>
	</div>
</div>

<Modal bind:this={deleteConfirmationModal} title="Delete all local data?">
	<p>Are you sure you want to delete all local data? This can’t be undone.</p>

	<svelte:fragment slot="controls" let:close>
		<Button kind="outlined" on:click={close}>Cancel</Button>
		<Button color="destructive" loading={isDeleting} on:click={onDeleteClicked}>Delete</Button>
	</svelte:fragment>
</Modal>
