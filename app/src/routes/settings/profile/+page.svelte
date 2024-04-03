<script lang="ts">
	import { CloudClient } from '$lib/backend/cloud';
	import { deleteAllData } from '$lib/backend/data';
	import Button from '$lib/components/Button.svelte';
	import Login from '$lib/components/Login.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import Spacer from '$lib/components/Spacer.svelte';
	import TextBox from '$lib/components/TextBox.svelte';
	import ThemeSelector from '$lib/components/ThemeSelector.svelte';
	import Toggle from '$lib/components/Toggle.svelte';
	import WelcomeSigninAction from '$lib/components/WelcomeSigninAction.svelte';
	import ContentWrapper from '$lib/components/settings/ContentWrapper.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { UserService } from '$lib/stores/user';
	import { getContext, getContextStoreBySymbol } from '$lib/utils/context';
	import * as toasts from '$lib/utils/toasts';
	import type { Writable } from 'svelte/store';
	import { goto } from '$app/navigation';

	const userService = getContext(UserService);
	const cloud = getContext(CloudClient);
	const user = userService.user;

	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);

	const fileTypes = ['image/jpeg', 'image/png'];

	let saving = false;
	let newName = '';
	let isDeleting = false;
	let loaded = false;

	$: userPicture = $user?.picture;

	let deleteConfirmationModal: Modal;

	$: if ($user && !loaded) {
		loaded = true;
		cloud.getUser($user?.access_token).then((cloudUser) => {
			cloudUser.github_access_token = $user?.github_access_token; // prevent overwriting with null
			userService.setUser(cloudUser);
		});
		newName = $user?.name || '';
	}

	async function onSubmit(e: SubmitEvent) {
		if (!$user) return;
		saving = true;

		const target = e.target as HTMLFormElement;
		const formData = new FormData(target);
		const picture = formData.get('picture') as File | undefined;

		try {
			const updatedUser = await cloud.updateUser($user.access_token, {
				name: newName,
				picture: picture
			});
			updatedUser.github_access_token = $user?.github_access_token; // prevent overwriting with null
			userService.setUser(updatedUser);
			toasts.success('Profile updated');
		} catch (e) {
			console.error(e);
			toasts.error('Failed to update user');
		}
		saving = false;
	}

	function onPictureChange(e: Event) {
		const target = e.target as HTMLInputElement;
		const file = target.files?.[0];

		if (file && fileTypes.includes(file.type)) {
			userPicture = URL.createObjectURL(file);
		} else {
			userPicture = $user?.picture;
			toasts.error('Please use a valid image file');
		}
	}

	async function onDeleteClicked() {
		isDeleting = true;
		try {
			deleteAllData();
			await userService.logout();
			// TODO: Delete user from observable!!!
			toasts.success('All data deleted');
			goto('/', { replaceState: true, invalidateAll: true });
		} catch (err: any) {
			console.error(err);
			toasts.error('Failed to delete project');
		} finally {
			deleteConfirmationModal.close();
			isDeleting = false;
		}
	}
</script>

<ContentWrapper title="Profile">
	{#if $user}
		<SectionCard>
			<form on:submit={onSubmit} class="profile-form">
				<label id="profile-picture" class="focus-state profile-pic-wrapper" for="picture">
					<input
						on:change={onPictureChange}
						type="file"
						id="picture"
						name="picture"
						accept={fileTypes.join('')}
						class="hidden-input"
					/>

					{#if $user.picture}
						<img class="profile-pic" src={userPicture} alt="" />
					{/if}

					<span class="profile-pic__edit-label text-base-11 text-semibold">Edit</span>
				</label>

				<div id="contact-info" class="contact-info">
					<div class="contact-info__fields">
						<TextBox label="Full name" bind:value={newName} required />
						<TextBox label="Email" bind:value={$user.email} readonly />
					</div>

					<Button style="pop" kind="solid" loading={saving}>Update profile</Button>
				</div>
			</form>
		</SectionCard>
	{:else}
		<WelcomeSigninAction />
		<Spacer />
	{/if}

	<SectionCard>
		<svelte:fragment slot="title">Theme</svelte:fragment>
		<ThemeSelector {userSettings} />
	</SectionCard>

	<SectionCard orientation="row" centerAlign>
		<svelte:fragment slot="title">Tab size</svelte:fragment>
		<svelte:fragment slot="caption">
			The number of spaces a tab is equal to when previewing code changes.
		</svelte:fragment>

		<svelte:fragment slot="actions">
			<TextBox
				type="number"
				width={100}
				textAlign="center"
				value={$userSettings.tabSize.toString()}
				minVal={1}
				maxVal={8}
				showCountActions
				on:change={(e) => {
					userSettings.update((s) => ({
						...s,
						tabSize: parseInt(e.detail) || $userSettings.tabSize
					}));
				}}
				placeholder={$userSettings.tabSize.toString()}
			/>
		</svelte:fragment>
	</SectionCard>

	<SectionCard labelFor="hoverScrollbarVisability" orientation="row">
		<svelte:fragment slot="title">Dynamic scrollbar visibility on hover</svelte:fragment>
		<svelte:fragment slot="caption">
			When turned on, this feature shows the scrollbar automatically when you hover over the scroll
			area, even if you're not actively scrolling. By default, the scrollbar stays hidden until you
			start scrolling.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle
				id="hoverScrollbarVisability"
				checked={$userSettings.scrollbarVisabilityOnHover}
				on:change={() =>
					userSettings.update((s) => ({
						...s,
						scrollbarVisabilityOnHover: !s.scrollbarVisabilityOnHover
					}))}
			/>
		</svelte:fragment>
	</SectionCard>

	<Spacer />

	{#if $user}
		<SectionCard orientation="row">
			<svelte:fragment slot="title">Signing out</svelte:fragment>
			<svelte:fragment slot="caption">
				Ready to take a break? Click here to log out and unwind.
			</svelte:fragment>

			<Login />
		</SectionCard>
	{/if}

	<SectionCard orientation="row">
		<svelte:fragment slot="title">Remove all projects</svelte:fragment>
		<svelte:fragment slot="caption">
			You can delete all projects from the GitButler app.
			<br />
			Your code remains safe. it only clears the configuration.
		</svelte:fragment>

		<Button style="error" kind="soft" color="error" on:click={() => deleteConfirmationModal.show()}>
			Remove projects…
		</Button>

		<Modal bind:this={deleteConfirmationModal} width="small" title="Remove all projects">
			<p>Are you sure you want to remove all GitButler projects?</p>

			<svelte:fragment slot="controls" let:close>
				<Button style="error" kind="soft" loading={isDeleting} on:click={onDeleteClicked}
					>Remove</Button
				>
				<Button style="pop" kind="solid" on:click={close}>Cancel</Button>
			</svelte:fragment>
		</Modal>
	</SectionCard>
</ContentWrapper>

<style lang="postcss">
	.profile-form {
		display: flex;
		gap: var(--size-24);
	}

	.hidden-input {
		cursor: pointer;
		z-index: 1;
		position: absolute;
		width: 100%;
		height: 100%;
		opacity: 0;
	}

	.profile-pic-wrapper {
		position: relative;
		width: 100px;
		height: 100px;
		border-radius: var(--radius-m);
		overflow: hidden;
		background-color: var(--clr-scale-pop-70);
		transition: opacity var(--transition-medium);

		&:hover,
		&:focus-within {
			& .profile-pic__edit-label {
				opacity: 1;
			}

			& .profile-pic {
				opacity: 0.8;
			}
		}
	}

	.profile-pic {
		width: 100%;
		height: 100%;

		object-fit: cover;
		background-color: var(--clr-scale-pop-70);
	}

	.profile-pic__edit-label {
		position: absolute;
		bottom: var(--size-8);
		left: var(--size-8);
		color: var(--clr-core-ntrl-100);
		background-color: color-mix(in srgb, var(--clr-core-ntrl-0), transparent 30%);
		padding: var(--size-4) var(--size-6);
		border-radius: var(--radius-m);
		opacity: 0;
		transition: opacity var(--transition-medium);
	}

	.contact-info {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: var(--size-20);
		align-items: flex-end;
	}

	.contact-info__fields {
		width: 100%;
		display: flex;
		flex-direction: column;
		gap: var(--size-12);
	}
</style>
