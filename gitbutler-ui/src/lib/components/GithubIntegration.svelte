<script lang="ts">
	import { checkAuthStatus, initDeviceOauth } from '$lib/backend/github';
	import Button from '$lib/components/Button.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import { getAuthenticated } from '$lib/github/user';
	import { copyToClipboard } from '$lib/utils/clipboard';
	import type { UserService } from '$lib/stores/user';

	export let userService: UserService;
	export let minimal = false;
	export let disabled = false;

	$: user$ = userService.user$;

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
		checkAuthStatus({ deviceCode }).then(async (access_token) => {
			let u = $user$;
			if (u) {
				u.github_access_token = access_token;
				u.github_username = await getAuthenticated(access_token);
				userService.setUser(u);
			}
		});
	}

	function forgetGitHub(): void {
		let u = $user$;
		if (u) {
			u.github_access_token = '';
			u.github_username = '';
			userService.setUser(u);
		}
	}
</script>

{#if minimal}
	{#if $user$?.github_access_token}
		<Button {disabled} kind="filled" color="primary" on:click={forgetGitHub}>Forget</Button>
	{:else}
		<Button {disabled} kind="filled" color="primary" on:click={gitHubStartOauth}
			>Authenticate</Button
		>
	{/if}
{:else}
	<div class="flex items-center">
		<div class="flex-grow">
			<p>
				GitHub
				{#if $user$?.github_access_token}
					<span class="text-sm text-green-500">️✅ — already configured</span>
				{/if}
			</p>
			<p class="text-sm text-light-700 dark:text-dark-200">
				Allows you to view and create Pull Requests from GitButler.
			</p>
		</div>
		<div>
			<Button {disabled} kind="filled" color="primary" on:click={gitHubStartOauth}>
				{#if $user$?.github_access_token}
					Reauthenticate
				{:else}
					Authenticate
				{/if}
			</Button>
		</div>
	</div>
{/if}

<Modal
	on:close={() => gitHubOauthCheckStatus(deviceCode)}
	bind:this={gitHubOauthModal}
	title="Authenticate with GitHub"
>
	<div class="flex flex-col gap-4">
		<div class="flex items-center gap-2">
			<span class="flex-grow">1️⃣ Copy the following verification code: </span>
			<input
				bind:value={userCode}
				class="
						whitespece-pre h-6 w-24 select-all rounded border border-light-200 bg-white font-mono dark:border-dark-400 dark:bg-dark-700"
			/>

			<Button {disabled} kind="outlined" color="primary" on:click={() => copyToClipboard(userCode)}>
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
		<Button {disabled} color="primary" on:click={close}>Done</Button>
	</svelte:fragment>
</Modal>
