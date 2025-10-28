<script lang="ts">
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { env } from '$env/dynamic/public';

	const routes = inject(WEB_ROUTES_SERVICE);
	const userService = inject(USER_SERVICE);
	const user = $derived(userService.user);

	function logout() {
		window.location.href = `${env.PUBLIC_APP_HOST}cloud/logout`;
	}

	function login() {
		window.location.href = `${env.PUBLIC_APP_HOST}cloud/login?callback=${window.location.href}`;
	}
</script>

<div class="navigation">
	<div class="domains">
		<a href="/" class="main-nav" aria-label="main nav" title="Home">
			<svg
				xmlns="http://www.w3.org/2000/svg"
				width="23"
				height="24"
				viewBox="0 0 23 24"
				fill="none"
			>
				<path d="M0 24V0L11.4819 10.5091L23 0V24L11.4819 13.5273L0 24Z" fill="black" />
			</svg>
		</a>

		{#if $user}
			<a class="nav-link nav-button" href="/organizations" aria-label="organizations">
				<svg
					xmlns="http://www.w3.org/2000/svg"
					width="16"
					height="16"
					viewBox="0 0 16 16"
					fill="none"
				>
					<g clip-path="url(#clip0_4626_21530)">
						<path
							fill-rule="evenodd"
							clip-rule="evenodd"
							d="M-0.0524597 0.542423L0.69662 0.505282L10.6844 0.0100717L11.4334 -0.0270691L11.4706 0.722011L11.5959 3.25H14H14.75V4V9.25C14.75 10.7451 14.0461 12.1529 12.85 13.05L11.85 13.8C10.1611 15.0667 7.83889 15.0667 6.15 13.8L5.15 13.05C4.47694 12.5452 3.95973 11.8787 3.63609 11.1259L2.32355 10.236C1.08858 9.39873 0.31878 8.02983 0.244893 6.53962L-0.0153189 1.2915L-0.0524597 0.542423ZM10.0096 1.54537L10.0941 3.25H4H3.25V4V9.05187L3.16529 8.99444C2.32031 8.42157 1.79361 7.48496 1.74305 6.46534L1.51998 1.9663L10.0096 1.54537ZM4.75 4.75V9.25C4.75 10.273 5.23163 11.2362 6.05 11.85L7.05 12.6C8.20555 13.4667 9.79444 13.4667 10.95 12.6L11.95 11.85C12.7684 11.2362 13.25 10.273 13.25 9.25V4.75H4.75ZM6 8.25H8V6.75H6V8.25ZM12 8.25H10V6.75H12V8.25ZM8 11.25H10V9.75H8V11.25Z"
							fill="black"
						/>
					</g>
					<defs>
						<clipPath id="clip0_4626_21530">
							<rect width="16" height="16" fill="white" />
						</clipPath>
					</defs>
				</svg>
			</a>

			<a
				class="nav-link nav-button"
				href={routes.projectsPath()}
				aria-label="projects"
				title="Projects"
			>
				<svg
					xmlns="http://www.w3.org/2000/svg"
					width="16"
					height="16"
					viewBox="0 0 16 16"
					fill="none"
				>
					<g clip-path="url(#clip0_2851_11817)">
						<path
							fill-rule="evenodd"
							clip-rule="evenodd"
							d="M9.23744 1.17675C8.55402 0.493331 7.44598 0.493329 6.76257 1.17675L1.17678 6.76253C0.493362 7.44595 0.49336 8.55399 1.17678 9.23741L6.76257 14.8232C7.44598 15.5066 8.55402 15.5066 9.23744 14.8232L14.8232 9.23741C15.5066 8.55399 15.5066 7.44595 14.8232 6.76254L9.23744 1.17675ZM7.82323 2.23741C7.92086 2.13978 8.07915 2.13978 8.17678 2.23741L13.7626 7.8232C13.8602 7.92083 13.8602 8.07912 13.7626 8.17675L8.17678 13.7625C8.07915 13.8602 7.92086 13.8602 7.82323 13.7625L2.23744 8.17675C2.13981 8.07912 2.13981 7.92083 2.23744 7.82319L5.5 4.56063L7.43934 6.49997L4.96967 8.96964L6.03033 10.0303L8.5 7.56063L10.4697 9.5303L11.5303 8.46964L9.03033 5.96964L6.56066 3.49997L7.82323 2.23741Z"
							fill="black"
						/>
					</g>
					<defs>
						<clipPath id="clip0_2851_11817">
							<rect width="16" height="16" fill="white" />
						</clipPath>
					</defs>
				</svg>
			</a>
		{/if}
	</div>
	<div class="nav__bottom">
		<button
			type="button"
			class="nav__bottom--button profile-link"
			onclick={() => {
				if ($user) {
					logout();
				} else {
					login();
				}
			}}
		>
			{#if $user}
				<a href="/user"> [u] </a>
				<svg
					xmlns="http://www.w3.org/2000/svg"
					width="16"
					height="16"
					viewBox="0 0 16 16"
					fill="none"
				>
					<path
						d="M2 10L7.55279 12.7764C7.83431 12.9172 8.16569 12.9172 8.44721 12.7764L14 10"
						stroke="#B4AFAC"
						stroke-width="1.5"
					/>
					<path
						d="M2 6L7.55279 3.22361C7.83431 3.08284 8.16569 3.08284 8.44721 3.22361L14 6"
						stroke="#B4AFAC"
						stroke-width="1.5"
					/>
				</svg>
			{:else}
				<svg
					xmlns="http://www.w3.org/2000/svg"
					width="16"
					height="16"
					viewBox="0 0 16 16"
					fill="none"
				>
					<path
						fill-rule="evenodd"
						clip-rule="evenodd"
						d="M2.25 5C2.25 2.92893 3.92893 1.25 6 1.25H10C12.0711 1.25 13.75 2.92893 13.75 5V11C13.75 13.0711 12.0711 14.75 10 14.75H6C3.92893 14.75 2.25 13.0711 2.25 11H3.75C3.75 12.2426 4.75736 13.25 6 13.25H10C11.2426 13.25 12.25 12.2426 12.25 11V5C12.25 3.75736 11.2426 2.75 10 2.75H6C4.75736 2.75 3.75 3.75736 3.75 5H2.25Z"
						fill="black"
					/>
					<path
						fill-rule="evenodd"
						clip-rule="evenodd"
						d="M7.03033 4.46967L10.5607 8L7.03033 11.5303L5.96967 10.4697L7.68934 8.75H0.5V7.25H7.68934L5.96967 5.53033L7.03033 4.46967Z"
						fill="black"
					/>
				</svg>
			{/if}
		</button>
		<a class="nav-link nav-button" href="/downloads" aria-label="downloads" title="Downloads">
			<svg
				xmlns="http://www.w3.org/2000/svg"
				width="16"
				height="16"
				viewBox="0 0 16 16"
				fill="none"
			>
				<g opacity="0.5">
					<path
						fill-rule="evenodd"
						clip-rule="evenodd"
						d="M4.75 5.25C4.75 5.11193 4.86193 5 5 5H11C11.1381 5 11.25 5.11193 11.25 5.25V10.25H4.75V5.25ZM3.25 10.25V5.25C3.25 4.2835 4.0335 3.5 5 3.5H11C11.9665 3.5 12.75 4.2835 12.75 5.25V10.25H14V11.25C14 11.8023 13.5523 12.25 13 12.25H3C2.44772 12.25 2 11.8023 2 11.25V10.25H3.25Z"
						fill="black"
					/>
				</g>
			</svg>
		</a>
		<a
			class="nav-link email"
			href="mailto:hello@gitbutler.com"
			aria-label="contact us"
			title="Contact Us"
		>
			<svg
				xmlns="http://www.w3.org/2000/svg"
				width="16"
				height="16"
				viewBox="0 0 16 16"
				fill="none"
			>
				<path
					fill-rule="evenodd"
					clip-rule="evenodd"
					d="M0 5.83C0 4.12153 0 3.26729 0.33776 2.61708C0.622386 2.06915 1.06915 1.62239 1.61708 1.33776C2.26729 1 3.12153 1 4.83 1L11.17 1C12.8785 1 13.7327 1 14.3829 1.33776C14.9309 1.62239 15.3776 2.06915 15.6622 2.61708C16 3.26729 16 4.12153 16 5.83V10.17C16 11.8785 16 12.7327 15.6622 13.3829C15.3776 13.9309 14.9309 14.3776 14.3829 14.6622C13.7327 15 12.8785 15 11.17 15L4.83 15C3.12153 15 2.26729 15 1.61708 14.6622C1.06915 14.3776 0.622386 13.9309 0.33776 13.3829C0 12.7327 0 11.8785 0 10.17L0 5.83ZM4.83 2.5L11.17 2.5C12.0494 2.5 12.6173 2.50121 13.0492 2.53707C13.4631 2.57145 13.6161 2.62976 13.6915 2.66888C13.9654 2.81119 14.1888 3.03457 14.3311 3.30854C14.352 3.34874 14.3783 3.41107 14.4036 3.5234L8.81349 8.31493C8.34537 8.71617 7.65462 8.71617 7.18651 8.31493L1.5964 3.52341C1.62165 3.41107 1.648 3.34874 1.66888 3.30854C1.81119 3.03457 2.03457 2.81119 2.30854 2.66888C2.38385 2.62976 2.53687 2.57145 2.95083 2.53707C3.38275 2.50121 3.95059 2.5 4.83 2.5ZM1.50025 5.41662C1.50003 5.54634 1.5 5.68387 1.5 5.83L1.5 10.17C1.5 11.0494 1.50121 11.6173 1.53707 12.0492C1.57145 12.4631 1.62976 12.6161 1.66888 12.6915C1.81119 12.9654 2.03457 13.1888 2.30854 13.3311C2.38385 13.3702 2.53687 13.4285 2.95083 13.4629C3.38275 13.4988 3.95059 13.5 4.83 13.5L11.17 13.5C12.0494 13.5 12.6173 13.4988 13.0492 13.4629C13.4631 13.4285 13.6161 13.3702 13.6915 13.3311C13.9654 13.1888 14.1888 12.9654 14.3311 12.6915C14.3702 12.6161 14.4285 12.4631 14.4629 12.0492C14.4988 11.6173 14.5 11.0494 14.5 10.17L14.5 5.83C14.5 5.68387 14.5 5.54634 14.4997 5.41661L9.78967 9.45382C8.75982 10.3365 7.24017 10.3365 6.21032 9.45382L1.50025 5.41662Z"
					fill="#867E79"
				/>
			</svg>
		</a>
	</div>
</div>

<style>
	/* sidebar, vertical nav */
	.navigation {
		display: flex;
		flex-direction: column;
		align-items: center;
		align-items: center;

		justify-content: space-between;
		width: 64px;

		height: 100vh;
		padding: 24px;
		gap: 16px;
		background-color: var(--color-background);
	}

	.main-nav {
		display: flex;
		align-items: center;
		justify-content: center;
		margin-bottom: 36px;
	}

	.nav__bottom {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.profile-link {
		display: flex;
		flex-direction: column;
		align-items: center;
		align-self: stretch;
		justify-content: center;
		padding: 4px 4px 7px 4px;
		gap: 4px;
		border: 1px solid var(--border-2, #d4d0ce);
		border-radius: 10px;
	}
	.nav-button {
		margin-bottom: 8px;
		border: 1px solid var(--border-2, #d4d0ce);
		border-radius: 10px;
		opacity: 0.8;
	}
	.nav-link {
		display: flex;
		align-items: center;
		align-self: stretch;
		justify-content: center;
		height: var(--button, 28px);
		padding: var(--4, 4px) var(--6, 6px);
		gap: var(--4, 4px);
	}
</style>
