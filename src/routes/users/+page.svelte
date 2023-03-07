<script lang="ts">
	import { Login } from '$lib/components';
	import type { PageData } from './$types';
	import { IconRotateClockwise2 } from '@tabler/icons-svelte';
	import { log, toasts } from '$lib';

	export let data: PageData;
	const { user, api } = data;

	$: saving = false;

	let userName = $user?.name;
	let userPicture = $user?.picture;

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

	const onSubmit = async (e: SubmitEvent) => {
		if (!$user) return;
		saving = true;

		const target = e.target as HTMLFormElement;
		const formData = new FormData(target);
		const name = formData.get('name') as string | undefined;
		const picture = formData.get('picture') as File | undefined;

		try {
			$user = await api.user.update($user.access_token, {
				name,
				picture: picture
			});
			toasts.success('Profile updated');
		} catch (e) {
			log.error(e);
			toasts.error('Failed to update user');
		}

		saving = false;
	};
</script>

<div class="mx-auto p-4">
	<div class="mx-auto max-w-xl p-4">
		{#if $user}
			<div class="flex flex-col gap-6 text-zinc-100">
				<header class="flex items-center justify-between">
					<div class="flex flex-col">
						<h2 class="text-xl font-medium">GitButler Cloud Account</h2>
						<div class="text-sm text-zinc-300">Your online account details on gitbutler.com</div>
					</div>
					<Login {user} {api} />
				</header>

				<form
					on:submit={onSubmit}
					class="flex flex-row items-start justify-between gap-12 rounded-lg p-2"
				>
					<fields id="left" class="flex flex-1 flex-col gap-3">
						<div class="flex flex-col gap-1">
							<label for="name" class="text-zinc-400">Name</label>
							<input
								id="name"
								name="name"
								bind:value={userName}
								type="text"
								class="w-full rounded-lg border border-zinc-600 bg-black px-2 py-1 text-zinc-300"
								required
							/>
						</div>

						<div class="flex flex-col gap-1">
							<label for="email" class="text-zinc-400">Email</label>
							<input
								disabled
								id="email"
								name="email"
								bind:value={$user.email}
								type="text"
								class="w-full rounded-lg border border-zinc-600 bg-black px-2 py-1 text-zinc-300"
							/>
						</div>

						<footer class="pt-4">
							{#if saving}
								<div
									class="w-content flex w-32 flex-row items-center justify-center gap-1 rounded bg-blue-600 px-4 py-2 text-white"
								>
									<IconRotateClockwise2 class="h-5 w-5 animate-spin" />
									<span>Updating...</span>
								</div>
							{:else}
								<button
									type="submit"
									class="rounded-md bg-blue-600 px-4 py-2 text-white hover:bg-blue-700 focus:outline-none"
									>Update profile</button
								>
							{/if}
						</footer>
					</fields>

					<fields id="right" class="flex flex-col items-center gap-2">
						{#if $user.picture}
							<img
								class="h-28 w-28 rounded-full border-zinc-300"
								src={userPicture}
								alt="Your avatar"
							/>
						{/if}

						<label
							for="picture"
							class="font-sm -mt-6 -ml-16 cursor-pointer rounded-lg border border-zinc-600 bg-zinc-800 bg-zinc-800 px-2 text-center text-zinc-300 hover:bg-zinc-900 hover:text-zinc-50"
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
				</form>
			</div>
		{:else}
			<div class="flex flex-col items-center justify-items-center space-y-6 text-white">
				<div class="text-3xl font-bold text-white">Connect to GitButler Cloud</div>
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
					<Login {user} {api} />
				</div>
				<div class="text-center text-zinc-300">
					You will still need to give us permission for each project before we transfer any data to
					our servers. You can revoke this permission at any time.
				</div>
			</div>
		{/if}

		<div class="mt-8 flex flex-col border-t border-zinc-400 pt-4">
			<h2 class="text-lg font-medium text-zinc-100">Get Support</h2>
			<div class="text-sm text-zinc-300">
				If you have an issue or any questions, please email us.
			</div>
			<div class="mt-4">
				<a class="text-blue-200" href="mailto:hello@gitbutler.com">hello@gitbutler.com</a>
			</div>
		</div>
	</div>
</div>
