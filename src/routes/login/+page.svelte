<script lang="ts">
	import { Button, Login } from '$lib/components';
	import { goto } from '$app/navigation';
	import Logo from '$lib/assets/logo-310x310.png';
</script>

<div
	class="flex w-full items-center border-t border-light-400 bg-white dark:border-dark-500 dark:bg-dark-1000"
>
	<div
		class="flex h-full flex-1 flex-col items-end justify-center gap-y-4 border-r border-light-400 dark:border-dark-500 dark:bg-dark-800"
	>
		<div class="mr-20 ml-8 flex w-80 max-w-lg flex-col items-center gap-y-4">
			<img alt="logo" class="inline-block h-32 w-32" src={Logo} />
			<p class="text-2xl font-medium">Login to GitButler</p>
			<Login width="full-width" />
			<Button color="purple" width="full-width" kind="outlined" on:click={() => goto('/')}
				>Skip</Button
			>
		</div>
	</div>
	<div class="flex h-full flex-1 flex-col justify-center">
		<div class="ml-20 mr-8 flex w-80 max-w-lg flex-col gap-y-4">
			<h1 class="text-2xl font-medium">More than just version control</h1>
			<p>
				GitButler is a tool to help you manage all the local work you do on your code projects.<br
				/><br />Think of us as a code concierge, a smart assistant for all the coding related tasks
				you need to do every day.
			</p>
			<div class="flex gap-x-4">
				<svg
					width="25"
					height="24"
					viewBox="0 0 25 24"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
					class="h-6 w-6 flex-shrink-0"
				>
					<path
						d="M21.4999 16.8107C21.4999 17.6747 20.5669 18.2157 19.8169 17.7877L12.7089 13.7257C12.5369 13.6272 12.3939 13.4851 12.2944 13.3136C12.195 13.1421 12.1426 12.9474 12.1426 12.7492C12.1426 12.5509 12.195 12.3562 12.2944 12.1847C12.3939 12.0133 12.5369 11.8711 12.7089 11.7727L19.8169 7.71066C19.988 7.61292 20.1818 7.56184 20.3789 7.56253C20.5759 7.56322 20.7693 7.61565 20.9397 7.71458C21.1102 7.81351 21.2516 7.95546 21.3499 8.12624C21.4482 8.29701 21.5 8.49061 21.4999 8.68766V16.8107ZM11.7499 16.8107C11.7499 17.6747 10.8169 18.2157 10.0669 17.7877L2.95894 13.7257C2.78688 13.6272 2.64387 13.4851 2.54442 13.3136C2.44496 13.1421 2.39258 12.9474 2.39258 12.7492C2.39258 12.5509 2.44496 12.3562 2.54442 12.1847C2.64387 12.0133 2.78688 11.8711 2.95894 11.7727L10.0669 7.70966C10.238 7.61192 10.4318 7.56084 10.6289 7.56153C10.8259 7.56222 11.0193 7.61465 11.1897 7.71358C11.3602 7.81251 11.5016 7.95446 11.5999 8.12524C11.6982 8.29601 11.75 8.48961 11.7499 8.68666V16.8097V16.8107Z"
						stroke="currentColor"
						stroke-width="1.5"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>

				<span class="inline-block align-middle">
					Automatically recording everything you do in any o your butlered projects.
				</span>
			</div>
			<div class="flex gap-x-4">
				<svg
					width="25"
					height="24"
					viewBox="0 0 25 24"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
					class="h-6 w-6 flex-shrink-0"
				>
					<path
						d="M21.5 11.25V19.5C21.5 19.8978 21.342 20.2794 21.0607 20.5607C20.7794 20.842 20.3978 21 20 21H5.75C5.35218 21 4.97064 20.842 4.68934 20.5607C4.40804 20.2794 4.25 19.8978 4.25 19.5V11.25M12.5 4.875C12.5 4.35583 12.346 3.84831 12.0576 3.41663C11.7692 2.98495 11.3592 2.6485 10.8795 2.44982C10.3999 2.25114 9.87209 2.19915 9.36289 2.30044C8.85369 2.40173 8.38596 2.65173 8.01884 3.01885C7.65173 3.38596 7.40172 3.85369 7.30044 4.36289C7.19915 4.87209 7.25114 5.39989 7.44982 5.87954C7.6485 6.3592 7.98495 6.76917 8.41663 7.05761C8.84831 7.34605 9.35582 7.5 9.875 7.5H12.5M12.5 4.875V7.5M12.5 4.875C12.5 4.35583 12.654 3.84831 12.9424 3.41663C13.2308 2.98495 13.6408 2.6485 14.1205 2.44982C14.6001 2.25114 15.1279 2.19915 15.6371 2.30044C16.1463 2.40173 16.614 2.65173 16.9812 3.01885C17.3483 3.38596 17.5983 3.85369 17.6996 4.36289C17.8008 4.87209 17.7489 5.39989 17.5502 5.87954C17.3515 6.3592 17.0151 6.76917 16.5834 7.05761C16.1517 7.34605 15.6442 7.5 15.125 7.5H12.5M12.5 7.5V21M3.875 11.25H21.875C22.496 11.25 23 10.746 23 10.125V8.625C23 8.004 22.496 7.5 21.875 7.5H3.875C3.254 7.5 2.75 8.004 2.75 8.625V10.125C2.75 10.746 3.254 11.25 3.875 11.25Z"
						stroke="currentColor"
						stroke-width="1.5"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>
				<span class="inline-block align-middle">
					Simplifying all your Git work, including comitting, branching and pushing, to be easy and
					intuitive.
				</span>
			</div>
			<div class="flex gap-x-4">
				<svg
					width="25"
					height="24"
					viewBox="0 0 25 24"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
					class="h-6 w-6 flex-shrink-0"
				>
					<path
						d="M16.25 15.75L13.761 13.261M13.761 13.261C14.0744 12.9476 14.323 12.5755 14.4926 12.1661C14.6622 11.7566 14.7495 11.3177 14.7495 10.8745C14.7495 10.4313 14.6622 9.99241 14.4926 9.58294C14.323 9.17346 14.0744 8.8014 13.761 8.488C13.4476 8.1746 13.0755 7.926 12.6661 7.75639C12.2566 7.58678 11.8177 7.49948 11.3745 7.49948C10.9313 7.49948 10.4924 7.58678 10.0829 7.75639C9.67346 7.926 9.3014 8.1746 8.988 8.488C8.35493 9.12107 7.99927 9.9797 7.99927 10.875C7.99927 11.7703 8.35493 12.6289 8.988 13.262C9.62107 13.8951 10.4797 14.2507 11.375 14.2507C12.2703 14.2507 13.1289 13.8951 13.762 13.262L13.761 13.261ZM21.5 12C21.5 13.1819 21.2672 14.3522 20.8149 15.4442C20.3626 16.5361 19.6997 17.5282 18.864 18.364C18.0282 19.1997 17.0361 19.8626 15.9442 20.3149C14.8522 20.7672 13.6819 21 12.5 21C11.3181 21 10.1478 20.7672 9.05585 20.3149C7.96392 19.8626 6.97177 19.1997 6.13604 18.364C5.30031 17.5282 4.63738 16.5361 4.18508 15.4442C3.73279 14.3522 3.5 13.1819 3.5 12C3.5 9.61305 4.44821 7.32387 6.13604 5.63604C7.82387 3.94821 10.1131 3 12.5 3C14.8869 3 17.1761 3.94821 18.864 5.63604C20.5518 7.32387 21.5 9.61305 21.5 12Z"
						stroke="currentColor"
						stroke-width="1.5"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>
				<span class="inline-block align-middle">
					Helping you not just search for strings or past commits, but find useful context in the
					story of your code.
				</span>
			</div>
		</div>
	</div>
</div>
