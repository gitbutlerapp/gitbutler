<script lang="ts">
	import ShareIssueModal from '$components/ShareIssueModal.svelte';
	import { Project } from '$lib/project/project';
	import { DesktopRoutesService } from '$lib/routes/routes.svelte';
	import { User } from '$lib/user/user';
	import { getContextStore } from '@gitbutler/shared/context';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { slide } from 'svelte/transition';
	import { goto } from '$app/navigation';

	const routes = getContext(DesktopRoutesService);
	const project = getContext(Project);
	const user = getContextStore(User);

	let contextTriggerButton = $state<HTMLDivElement>();
	let contextMenuEl = $state<ContextMenu>();
	let shareIssueModal = $state<ShareIssueModal>();
</script>

<nav class="sidebar">
	<div class="top">
		<div class="">
			{#if routes.isWorkspacePath}
				<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
			{/if}
			<Button
				kind="outline"
				onclick={() => goto(routes.workspacePath(project.id))}
				width={34}
				class={['btn-square', routes.isWorkspacePath && 'btn-active']}
			>
				<svg
					viewBox="0 0 16 13"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
					class={[routes.isWorkspacePath && 'stroke-orange']}
				>
					<path
						d="M2 12L3.5 7.5M14 12L12.5 7.5M12.5 7.5L11 3H5L3.5 7.5M12.5 7.5H3.5"
						stroke-width="1.5"
					/>
					<path
						d="M1.24142 3H14.7586C14.8477 3 14.8923 2.89229 14.8293 2.82929L13.0293 1.02929C13.0105 1.01054 12.9851 1 12.9586 1H3.04142C3.0149 1 2.98946 1.01054 2.97071 1.02929L1.17071 2.82929C1.10771 2.89229 1.15233 3 1.24142 3Z"
						stroke-width="1.5"
					/>
				</svg>
			</Button>
		</div>
		<div class="">
			{#if routes.isBranchesPath}
				<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
			{/if}
			<Button
				kind="outline"
				onclick={() => goto(routes.branchesPath(project.id))}
				width={34}
				class={['btn-square', routes.isBranchesPath && 'btn-active']}
			>
				<svg
					viewBox="0 0 16 14"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
					class={[routes.isBranchesPath && 'stroke-purple']}
				>
					<path d="M5 3L11 3" stroke-width="1.5" />
					<path
						d="M3 5L3 7.17157C3 7.70201 3.21071 8.21071 3.58579 8.58579L5.41421 10.4142C5.78929 10.7893 6.29799 11 6.82843 11L11.5 11"
						stroke-width="1.5"
					/>
					<rect
						x="15"
						y="1"
						width="4"
						height="4"
						transform="rotate(90 15 1)"
						stroke-width="1.5"
						class={[routes.isBranchesPath && 'fill-purple']}
					/>
					<rect
						x="15"
						y="9"
						width="4"
						height="4"
						transform="rotate(90 15 9)"
						stroke-width="1.5"
						class={[routes.isBranchesPath && 'fill-purple']}
					/>
					<rect
						x="5"
						y="1"
						width="4"
						height="4"
						transform="rotate(90 5 1)"
						stroke-width="1.5"
						class={[routes.isBranchesPath && 'fill-purple']}
					/>
				</svg>
			</Button>
		</div>
		<div class="">
			{#if routes.isTargetPath}
				<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
			{/if}
			<Button
				kind="outline"
				onclick={() => goto(routes.targetPath(project.id))}
				width={34}
				class={['btn-square', routes.isTargetPath && 'btn-active']}
			>
				<svg
					viewBox="0 0 18 16"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
					class={[routes.isTargetPath && 'fill-orange stroke-orange']}
				>
					<path
						d="M10.6906 1C12.1197 1 13.4402 1.7624 14.1547 3L15.8453 5.9282C16.5598 7.16581 16.5598 8.6906 15.8453 9.9282L14.1547 12.8564C13.4402 14.094 12.1197 14.8564 10.6906 14.8564H7.3094C5.88034 14.8564 4.55983 14.094 3.8453 12.8564L2.1547 9.9282C1.44017 8.6906 1.44017 7.16581 2.1547 5.9282L3.8453 3C4.55983 1.7624 5.88034 1 7.3094 1H10.6906Z"
						stroke-width="1.5"
						class={[routes.isTargetPath && 'stroke-orange']}
					/>
					<path
						d="M9 14.5V10.5M9 5V1"
						stroke-width="1.5"
						class={[routes.isTargetPath && 'stroke-white']}
					/>
					<path
						d="M2.25 7.75L6.25 7.75M11.75 7.75L15.75 7.75"
						stroke-width="1.5"
						class={[routes.isTargetPath && 'stroke-white']}
					/>
					<circle cx="9" cy="8" r="1" class={[routes.isTargetPath && 'stroke-white']} />
				</svg>
			</Button>
		</div>
		<div class="">
			{#if routes.isHistoryPath}
				<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
			{/if}
			<Button
				kind="outline"
				onclick={() => goto(routes.historyPath(project.id))}
				width={34}
				class={['btn-square', routes.isHistoryPath && 'btn-active']}
			>
				<svg
					viewBox="0 0 18 18"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
					stroke-width="1.5"
					style="padding: 1px;"
				>
					<rect width="18" height="18" rx="6" class={[routes.isHistoryPath && 'fill-yellow']} />
					<path
						d="M8 3V10H13"
						stroke-width="1.5"
						class={[routes.isHistoryPath && 'stroke-black']}
					/>
				</svg>
			</Button>
		</div>
	</div>
	<div class="bottom" bind:this={contextTriggerButton}>
		<div class="">
			{#if routes.isSettingsPath}
				<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
			{/if}
			<Button
				icon="settings"
				kind="outline"
				onclick={() => goto(routes.settingsPath(project.id))}
				width={34}
				class="btn-square"
			/>
		</div>
		<Button
			kind="outline"
			width={34}
			class="btn-height-auto"
			onclick={() => {
				contextMenuEl?.toggle();
			}}
		>
			<div class="user-button">
				{#if $user?.picture}
					<img
						class="profile-picture"
						src={$user.picture}
						alt="Avatar"
						referrerpolicy="no-referrer"
					/>
				{:else}
					<div class="anon-icon">
						<Icon name="profile" />
					</div>
				{/if}
				<Icon name="select-chevron" />
			</div>
		</Button>

		<Button
			icon="keyboard"
			kind="ghost"
			style="neutral"
			tooltip="Keyboard Shortcuts"
			tooltipPosition="top"
			tooltipAlign="start"
			width={34}
			class="fill-text-2"
		/>
		<Button
			icon="mail"
			kind="ghost"
			tooltip="Share feedback"
			tooltipPosition="top"
			tooltipAlign="start"
			width={34}
			class="fill-text-2"
			onclick={() => {
				shareIssueModal.show();
			}}
		/>
	</div>
</nav>

<ContextMenu
	bind:this={contextMenuEl}
	leftClickTrigger={contextTriggerButton}
	verticalAlign="bottom"
	horizontalAlign="left"
>
	<ContextMenuSection>
		<ContextMenuItem
			label="Preferences"
			onclick={() => {
				contextMenuEl?.close();
			}}
		/>
	</ContextMenuSection>
	<ContextMenuSection title="Theme (⌘K)">
		<ContextMenuItem
			label="Dark"
			onclick={async () => {
				contextMenuEl?.close();
			}}
		/>
		<ContextMenuItem
			label="Light"
			onclick={async () => {
				contextMenuEl?.close();
			}}
		/>
		<ContextMenuItem
			label="System"
			onclick={async () => {
				contextMenuEl?.close();
			}}
		/>
	</ContextMenuSection>
	<ContextMenuSection>
		<ContextMenuItem
			label="Logout"
			onclick={async () => {
				contextMenuEl?.close();
			}}
		/>
	</ContextMenuSection>
</ContextMenu>

<ShareIssueModal bind:this={shareIssueModal} />

<style>
	.sidebar {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		justify-content: space-between;
		height: 100%;
		width: 50px;
		padding: 16px;
	}

	.top,
	.bottom {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.user-button {
		display: flex;
		flex-direction: column;
		justify-content: center;
		align-items: center;
		width: 34px;
	}

	.active-page-indicator {
		content: '';
		position: absolute;
		left: 0;
		width: 12px;
		height: 18px;
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-pop-element);
		transform: translateX(-50%) translateY(50%);
	}

	svg {
		width: 16px;
		height: 16px;
		stroke: var(--clr-btn-ntrl-outline-text);
	}

	.stroke-white {
		stroke: #fff;
	}

	.fill-orange {
		fill: #ff9774;
	}

	.stroke-orange {
		stroke: #ff9774;
	}

	.fill-purple {
		fill: #a486c8;
	}

	.stroke-purple {
		stroke: #a486c8;
	}

	.fill-yellow {
		fill: #fbdb79;
		stroke: #fbdb79;
	}

	.stroke-black {
		stroke: #000 !important;
	}

	:global(.fill-text-2) {
		fill: var(--clr-text-2) !important;
		opacity: 0.5;
	}

	:global(.btn-height-auto) {
		height: auto !important;
		opacity: 0.7;
	}

	:global(.btn-square) {
		aspect-ratio: 1 / 1;
		height: unset !important;
		opacity: 0.7;
	}
	:global(.btn-square.btn-active) {
		opacity: 1 !important;
	}
</style>
