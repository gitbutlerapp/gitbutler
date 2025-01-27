<script lang="ts">
	import KeyboardShortcutsModal from '$components/KeyboardShortcutsModal.svelte';
	import ShareIssueModal from '$components/ShareIssueModal.svelte';
	import { Project } from '$lib/project/project';
	import { DesktopRoutesService } from '$lib/routes/routes.svelte';
	import { User } from '$lib/user/user';
	import { UserService } from '$lib/user/userService';
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
	let keyboardShortcutsModal = $state<KeyboardShortcutsModal>();

	const userService = getContext(UserService);
</script>

<nav class="sidebar">
	<div class="top">
		<div>
			{#if routes.isWorkspacePath}
				<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
			{/if}
			<Button
				kind="outline"
				onclick={() => goto(routes.workspacePath(project.id))}
				width={34}
				class={['btn-square', routes.isWorkspacePath && 'btn-active']}
			>
				<svg viewBox="0 0 16 13" fill="none" xmlns="http://www.w3.org/2000/svg">
					<path
						d="M2 12L3.5 7.5M14 12L12.5 7.5M12.5 7.5L11 3H5L3.5 7.5M12.5 7.5H3.5"
						stroke-width="1.5"
						stroke="var(--clr-workspace-legs)"
					/>
					<path
						d="M1.24142 3H14.7586C14.8477 3 14.8923 2.89229 14.8293 2.82929L13.0293 1.02929C13.0105 1.01054 12.9851 1 12.9586 1H3.04142C3.0149 1 2.98946 1.01054 2.97071 1.02929L1.17071 2.82929C1.10771 2.89229 1.15233 3 1.24142 3Z"
						stroke-width="1.5"
						stroke="var(--clr-workspace-top)"
						fill="var(--clr-workspace-top)"
					/>
				</svg>
			</Button>
		</div>
		<div>
			{#if routes.isBranchesPath}
				<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
			{/if}
			<Button
				kind="outline"
				onclick={() => goto(routes.branchesPath(project.id))}
				width={34}
				class={['btn-square', routes.isBranchesPath && 'btn-active']}
			>
				<svg viewBox="0 0 16 14" fill="none" xmlns="http://www.w3.org/2000/svg">
					<path d="M5 3L11 3" stroke-width="1.5" stroke="var(--clr-branches)" />
					<path
						d="M3 5L3 7.17157C3 7.70201 3.21071 8.21071 3.58579 8.58579L5.41421 10.4142C5.78929 10.7893 6.29799 11 6.82843 11L11.5 11"
						stroke-width="1.5"
						stroke="var(--clr-branches)"
					/>
					<rect
						x="15"
						y="1"
						width="4"
						height="4"
						transform="rotate(90 15 1)"
						stroke-width="1.5"
						fill="var(--clr-branches)"
						stroke="var(--clr-branches)"
					/>
					<rect
						x="15"
						y="9"
						width="4"
						height="4"
						transform="rotate(90 15 9)"
						stroke-width="1.5"
						fill="var(--clr-branches)"
						stroke="var(--clr-branches)"
					/>
					<rect
						x="5"
						y="1"
						width="4"
						height="4"
						transform="rotate(90 5 1)"
						stroke-width="1.5"
						fill="var(--clr-branches)"
						stroke="var(--clr-branches)"
					/>
				</svg>
			</Button>
		</div>
		<div>
			{#if routes.isTargetPath}
				<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
			{/if}
			<Button
				kind="outline"
				onclick={() => goto(routes.targetPath(project.id))}
				width={34}
				class={['btn-square', routes.isTargetPath && 'btn-active']}
			>
				<svg viewBox="0 0 18 16" fill="none" xmlns="http://www.w3.org/2000/svg">
					<path
						d="M10.6906 1C12.1197 1 13.4402 1.7624 14.1547 3L15.8453 5.9282C16.5598 7.16581 16.5598 8.6906 15.8453 9.9282L14.1547 12.8564C13.4402 14.094 12.1197 14.8564 10.6906 14.8564H7.3094C5.88034 14.8564 4.55983 14.094 3.8453 12.8564L2.1547 9.9282C1.44017 8.6906 1.44017 7.16581 2.1547 5.9282L3.8453 3C4.55983 1.7624 5.88034 1 7.3094 1H10.6906Z"
						stroke-width="1.5"
						stroke="var(--clr-target-bg)"
						fill="var(--clr-target-bg)"
					/>
					<path d="M9 14.5V10.5M9 5V1" stroke-width="1.5" stroke="var(--clr-target-lines)" />
					<path
						d="M2.25 7.75L6.25 7.75M11.75 7.75L15.75 7.75"
						stroke-width="1.5"
						stroke="var(--clr-target-lines)"
					/>
					<circle cx="9" cy="8" r="1" stroke="var(--clr-target-lines)" />
				</svg>
			</Button>
		</div>
		<div>
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
					<rect
						width="18"
						height="18"
						rx="6"
						stroke="var(--clr-history-bg)"
						fill="var(--clr-history-bg)"
					/>
					<path d="M8 3V10H13" stroke-width="1.5" stroke="var(--clr-history-arrows)" />
				</svg>
			</Button>
		</div>
	</div>
	<div class="bottom">
		<div class="bottom__primary-actions">
			<div>
				{#if routes.isProjectSettingsPath}
					<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
				{/if}
				<Button
					icon="settings"
					kind="outline"
					onclick={() => goto(routes.projectSettingsPath(project.id))}
					width={34}
					class={['btn-square', routes.isProjectSettingsPath && 'btn-active']}
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
				<div class="user-button" bind:this={contextTriggerButton}>
					<div class="user-icon">
						{#if $user?.picture}
							<img
								class="user-icon__image"
								src={$user.picture}
								alt=""
								referrerpolicy="no-referrer"
							/>
						{:else}
							<Icon name="profile" />
						{/if}
					</div>
					<svg
						width="16"
						height="16"
						viewBox="0 0 16 16"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
						class="user-button__select-icon"
					>
						<path
							d="M2 10L7.55279 12.7764C7.83431 12.9172 8.16569 12.9172 8.44721 12.7764L14 10"
							stroke-width="1.5"
						/>
						<path
							d="M2 6L7.55279 3.22361C7.83431 3.08284 8.16569 3.08284 8.44721 3.22361L14 6"
							stroke-width="1.5"
						/>
					</svg>
				</div></Button
			>
		</div>
		<div class="bottom__ghost-actions">
			<Button
				icon="keyboard"
				kind="ghost"
				style="neutral"
				tooltip="Keyboard Shortcuts"
				tooltipPosition="top"
				tooltipAlign="start"
				width={34}
				class="faded-btn"
				onclick={() => {
					keyboardShortcutsModal?.show();
				}}
			/>
			<Button
				icon="mail"
				kind="ghost"
				tooltip="Share feedback"
				tooltipPosition="top"
				tooltipAlign="start"
				width={34}
				class="faded-btn"
				onclick={() => {
					shareIssueModal?.show();
				}}
			/>
		</div>
	</div>
</nav>

<ContextMenu
	bind:this={contextMenuEl}
	leftClickTrigger={contextTriggerButton}
	side="right"
	verticalAlign="top"
>
	<ContextMenuSection>
		<ContextMenuItem
			label="Preferences"
			onclick={() => {
				goto('/settings/profile');
			}}
		/>
	</ContextMenuSection>
	<ContextMenuSection title="Theme (⌘K)">
		<ContextMenuItem
			label="Dark"
			onclick={async () => {
				// TODO
				contextMenuEl?.close();
			}}
		/>
		<ContextMenuItem
			label="Light"
			onclick={async () => {
				// TODO
				contextMenuEl?.close();
			}}
		/>
		<ContextMenuItem
			label="System"
			onclick={async () => {
				// TODO
				contextMenuEl?.close();
			}}
		/>
	</ContextMenuSection>
	<ContextMenuSection>
		<ContextMenuItem
			label="Logout"
			onclick={async () => {
				await userService.logout();
			}}
		/>
	</ContextMenuSection>
</ContextMenu>

<KeyboardShortcutsModal bind:this={keyboardShortcutsModal} />
<ShareIssueModal bind:this={shareIssueModal} />

<style lang="postcss">
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
	}
	.top {
		gap: 4px;
	}
	.bottom {
		gap: 16px;
	}
	.bottom__primary-actions {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.bottom__ghost-actions {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	/* USER BUTTON */
	.user-button {
		display: flex;
		flex-direction: column;
		justify-content: center;
		align-items: center;
		width: 34px;
		padding: 4px;
		gap: 4px;
	}

	.user-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 26px;
		height: 26px;
		background-color: var(--clr-core-pop-50);
		color: var(--clr-core-ntrl-100);
		border-radius: var(--radius-m);
		overflow: hidden;
	}

	.user-icon__image {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}

	.user-button__select-icon {
		stroke: var(--label-clr);
		opacity: var(--icon-opacity);
		transition: opacity var(--transition-fast);
	}

	/*  */
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

	/* OVERRIDE BUTTON STYLES */
	:global(.sidebar .btn-square) {
		--label-clr: var(--clr-btn-ntrl-outline-text);
		--icon-opacity: var(--opacity-btn-icon-outline);

		& svg {
			width: 16px;
			height: 16px;
			stroke: currentColor;
			opacity: var(--icon-opacity);
			transition: opacity var(--transition-fast);
		}

		&:hover {
			--icon-opacity: var(--opacity-btn-icon-outline-hover);
		}
	}
	:global(.sidebar .btn-height-auto) {
		height: auto;
		border-radius: var(--radius-ml);
		padding: 0;
	}
	:global(.sidebar .btn-square) {
		aspect-ratio: 1 / 1;
		height: unset;
		border-radius: var(--radius-ml);
	}
	:global(.sidebar .btn-square.btn-active) {
		--icon-opacity: 1;
		--btn-bg: var(--clr-core-ntrl-100);
		--opacity-btn-bg: 1;

		/* workspace icon */
		--clr-workspace-legs: #d96842;
		--clr-workspace-top: #ff9774;

		/* branches icon */
		--clr-branches: #a486c8;

		/* target icon */
		--clr-target-bg: #ff9774;
		--clr-target-lines: #fff;

		/* history icon */
		--clr-history-bg: #fbdb79;
		--clr-history-arrows: #0a0a0a;
	}

	:global(.sidebar .faded-btn) {
		--icon-opacity: 0.4;

		&:hover {
			--icon-opacity: 0.6;
		}
	}
</style>
