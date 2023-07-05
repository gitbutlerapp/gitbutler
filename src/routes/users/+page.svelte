<script lang="ts">
	import { Button, Modal, Login } from '$lib/components';
	import type { PageData } from './$types';
	import { log, stores, toasts } from '$lib';
	import { deleteAllData } from '$lib/api';
	import { goto } from '$app/navigation';
	import Logo from '$lib/assets/logo-310x310.png';
	import ThemeSelector from '../ThemeSelector.svelte';

	export let data: PageData;
	const { cloud } = data;

	const user = stores.user;

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
			.then(() => user.set(null))
			.then(() => toasts.success('All data deleted'))
			.catch((e) => {
				log.error(e);
				toasts.error('Failed to delete project');
			})
			.then(() => deleteConfirmationModal.close())
			.then(() => goto('/', { replaceState: true, invalidateAll: true }))
			.finally(() => (isDeleting = false));

	$: if (!$user) {
		goto('/login/');
	}
</script>

{#if $user}
	<div class="mx-auto h-fit w-full max-w-xl bg-light-200 py-10 dark:bg-dark-900">
		<div class="flex flex-col gap-y-8">
			<div class="flex w-full justify-between">
				<h2 class="text-2xl font-medium">GitButler Settings</h2>
				<Login />
			</div>
			<div class="h-[0.0625rem] bg-light-400 dark:bg-dark-700" />

			<div>
				<h2 class="mb-2 text-lg font-medium">GitButler Cloud</h2>
				<p class="">Your online account details on gitbutler.com</p>
			</div>

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
							class="hidden"
						/>
					</label>
				</div>

				<div id="contact-info" class="flex flex-1 flex-wrap">
					<div class="basis-1/2 pr-4">
						<label for="firstName" class="text-zinc-400">First name</label>
						<input
							autocomplete="off"
							autocorrect="off"
							spellcheck="false"
							name="firstName"
							bind:value={$user.name}
							type="text"
							class="w-full"
							placeholder="First name can't be empty"
							required
						/>
					</div>
					<div class="basis-1/2 pr-4">
						<label for="lastName" class="text-zinc-400">Last name</label>
						<input
							autocomplete="off"
							autocorrect="off"
							spellcheck="false"
							name="lastName"
							type="text"
							class="w-full"
							placeholder="Last name can't be empty"
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
							class="w-full"
						/>
					</div>
					<div class="mt-4 basis-full pr-4 text-right">
						<Button loading={saving} color="purple" type="submit">Update profile</Button>
					</div>
				</div>
			</form>
			<div class="h-[0.0625rem] bg-light-400 dark:bg-dark-700" />
			<div>
				<h2 class="mb-2 text-lg font-medium">Appearance</h2>
			</div>
			<div class="flex items-center">
				<div class="flex-grow">
					<p>Automatically generate AI hunk changes</p>
					<p class="text-sm text-light-700 dark:text-dark-200">
						GitButler AI will review your hunk changes and write a description of what happened
					</p>
				</div>
				<div>
					<label class="relative inline-flex cursor-pointer items-center">
						<input type="checkbox" value="" class="peer sr-only" />
						<div
							class="peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 dark:bg-gray-700 dark:border-gray-600 peer h-6 w-11 rounded-full bg-gray-400 after:absolute after:top-[2px] after:left-[2px] after:h-5 after:w-5 after:rounded-full after:border after:border-light-500 after:bg-white after:transition-all after:content-[''] peer-checked:bg-purple-600 peer-checked:after:translate-x-full peer-checked:after:border-white peer-focus:outline-none peer-focus:ring-4"
						/>
					</label>
				</div>
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

			<div class="h-[0.0625rem] bg-light-400 dark:bg-dark-700" />

			<div>
				<h2 class="mb-2 text-lg font-medium">Need help?</h2>
			</div>
			<div class="flex gap-x-4">
				<a
					href="https://discord.gg/wDKZCPEjXC"
					target="_blank"
					rel="noreferrer"
					class="flex-1 rounded border border-light-200 bg-white p-4 dark:border-dark-400 dark:bg-dark-700"
				>
					<p class="mb-2 font-medium">Join our Discorder</p>
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

			<div class="h-[0.0625rem] bg-light-400 dark:bg-dark-700" />

			<div class="flex flex-col gap-4">
				<Button color="destructive" kind="outlined" on:click={() => deleteConfirmationModal.show()}>
					Delete all data
				</Button>
			</div>

			<Modal bind:this={deleteConfirmationModal} title="Delete all local data?">
				<p>Are you sure you want to delete all local data? This can’t be undone.</p>

				<svelte:fragment slot="controls" let:close>
					<Button kind="outlined" on:click={close}>Cancel</Button>
					<Button color="destructive" loading={isDeleting} on:click={onDeleteClicked}>Delete</Button
					>
				</svelte:fragment>
			</Modal>
		</div>
	</div>
{/if}
