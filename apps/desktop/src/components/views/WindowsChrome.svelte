<script lang="ts">
	import AppHeader from "$components/views/AppHeader.svelte";
	import ChromeProjectSelector from "$components/views/ChromeProjectSelector.svelte";
	import { BACKEND } from "$lib/backend";
	import type { WindowChromeState } from "$lib/backend/backend";
	import { inject } from "@gitbutler/core/context";
	import { onMount } from "svelte";

	type Props = {
		projectId: string;
		projectTitle: string;
		actionsDisabled?: boolean;
	};

	type WindowMenu = {
		id: string;
		label: string;
	};

	const { projectId, projectTitle, actionsDisabled = false }: Props = $props();
	const backend = inject(BACKEND);

	const menus: WindowMenu[] = [
		{ id: "menu/file", label: "File" },
		{ id: "menu/edit", label: "Edit" },
		{ id: "menu/view", label: "View" },
		{ id: "menu/project", label: "Project" },
		{ id: "menu/help", label: "Help" },
	];

	let windowState = $state<WindowChromeState>({
		isFocused: true,
		isMaximized: false,
	});

	onMount(() => {
		let unlisten: (() => Promise<void>) | undefined;

		void (async () => {
			unlisten = await backend.listenToWindowChromeState((state) => {
				windowState = state;
			});
		})();

		return () => {
			void unlisten?.();
		};
	});

	async function popupMenu(menuId: string, event: MouseEvent) {
		const button = event.currentTarget;
		if (!(button instanceof HTMLElement)) return;

		const rect = button.getBoundingClientRect();
		await backend.invoke<void>("popup_window_menu", {
			id: menuId,
			position: {
				x: rect.left,
				y: rect.bottom + 4,
			},
		});
	}

	async function toggleMaximizeWindow() {
		await backend.toggleMaximizeWindow();
	}
</script>

<div
	class="windows-chrome"
	class:focused={windowState.isFocused}
	class:maximized={windowState.isMaximized}
>
	<div class="windows-titlebar">
		<div class="windows-titlebar__left">
			<div class="windows-titlebar__menus">
				{#each menus as menu}
					<button
						type="button"
						class="windows-menu-button text-12 text-semibold"
						onclick={(event) => void popupMenu(menu.id, event)}
					>
						{menu.label}
					</button>
				{/each}
			</div>
		</div>

		<div class="windows-titlebar__center">
			<div
				class="windows-titlebar__drag-surface"
				data-tauri-drag-region
				role="button"
				tabindex="-1"
				aria-label="Window title bar"
				ondblclick={() => void toggleMaximizeWindow()}
			></div>
			<div class="windows-titlebar__selector">
				<ChromeProjectSelector {projectId} {projectTitle} />
			</div>
			<div
				class="windows-titlebar__drag-surface"
				data-tauri-drag-region
				role="button"
				tabindex="-1"
				aria-label="Window title bar"
				ondblclick={() => void toggleMaximizeWindow()}
			></div>
		</div>

		<div class="windows-titlebar__controls">
			<button
				type="button"
				class="caption-button"
				aria-label="Minimize window"
				onclick={() => void backend.minimizeWindow()}
			>
				<span class="caption-icon caption-icon--minimize"></span>
			</button>
			<button
				type="button"
				class="caption-button"
				aria-label={windowState.isMaximized ? "Restore window" : "Maximize window"}
				onclick={() => void toggleMaximizeWindow()}
			>
				<span
					class="caption-icon"
					class:caption-icon--maximize={!windowState.isMaximized}
					class:caption-icon--restore={windowState.isMaximized}
				></span>
			</button>
			<button
				type="button"
				class="caption-button caption-button--close"
				aria-label="Close window"
				onclick={() => void backend.closeWindow()}
			>
				<span class="caption-icon caption-icon--close"></span>
			</button>
		</div>
	</div>

	<AppHeader {projectId} {projectTitle} {actionsDisabled} showProjectSelector={false} />
</div>

<style>
	.windows-chrome {
		display: flex;
		flex-direction: column;
	}

	.windows-titlebar {
		display: grid;
		grid-template-columns: max-content minmax(0, 1fr) max-content;
		align-items: center;
		padding: 8px 14px 2px;
		gap: 14px;
	}

	.windows-titlebar__left {
		display: flex;
		align-items: center;
		min-width: 0;
	}

	.windows-titlebar__menus {
		display: flex;
		align-items: center;
		gap: 2px;
		padding: 2px;
		border: 1px solid color-mix(in srgb, var(--border-2) 82%, transparent);
		border-radius: calc(var(--radius-button) + 2px);
		background: color-mix(in srgb, var(--bg-1) 76%, transparent);
	}

	.windows-menu-button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		height: 28px;
		padding: 0 10px;
		border: none;
		border-radius: var(--radius-s);
		background: transparent;
		color: var(--text-2);
		cursor: pointer;
		transition:
			background-color var(--transition-fast),
			color var(--transition-fast);
	}

	.windows-menu-button:hover {
		background-color: var(--hover-bg-2);
		color: var(--text-1);
	}

	.windows-menu-button:active {
		background-color: color-mix(in srgb, var(--hover-bg-2) 88%, var(--bg-1));
	}

	.windows-titlebar__center {
		display: flex;
		align-items: center;
		gap: 10px;
		min-width: 0;
	}

	.windows-titlebar__drag-surface {
		flex: 1;
		align-self: stretch;
		min-width: 24px;
	}

	.windows-titlebar__selector {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		min-width: 0;
		max-width: min(340px, 100%);
	}

	.windows-titlebar__controls {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.caption-button {
		display: inline-flex;
		position: relative;
		align-items: center;
		justify-content: center;
		width: 42px;
		height: 28px;
		padding: 0;
		border: none;
		border-radius: var(--radius-s);
		background: transparent;
		color: var(--text-2);
		cursor: pointer;
		transition:
			background-color var(--transition-fast),
			color var(--transition-fast);
	}

	.caption-button:hover {
		background-color: var(--hover-bg-2);
		color: var(--text-1);
	}

	.caption-button--close:hover {
		background-color: #c42b1c;
		color: white;
	}

	.caption-icon {
		display: inline-block;
		position: relative;
		width: 10px;
		height: 10px;
	}

	.caption-icon--minimize::after {
		content: "";
		position: absolute;
		right: 0;
		bottom: 1px;
		left: 0;
		height: 1.5px;
		background: currentColor;
	}

	.caption-icon--maximize {
		box-sizing: border-box;
		border: 1.5px solid currentColor;
		border-top-width: 2px;
	}

	.caption-icon--restore::before,
	.caption-icon--restore::after {
		content: "";
		position: absolute;
		box-sizing: border-box;
		width: 8px;
		height: 8px;
		border: 1.5px solid currentColor;
		background: var(--bg-2);
	}

	.caption-icon--restore::before {
		top: 1px;
		right: 0;
		border-top-width: 2px;
	}

	.caption-icon--restore::after {
		bottom: 0;
		left: 0;
		border-top-width: 2px;
	}

	.caption-icon--close::before,
	.caption-icon--close::after {
		content: "";
		position: absolute;
		top: 4px;
		left: 0;
		width: 10px;
		height: 1.5px;
		background: currentColor;
		transform-origin: center;
	}

	.caption-icon--close::before {
		transform: rotate(45deg);
	}

	.caption-icon--close::after {
		transform: rotate(-45deg);
	}

	.windows-chrome:not(.focused) .windows-menu-button,
	.windows-chrome:not(.focused) .caption-button,
	.windows-chrome:not(.focused) .windows-titlebar__center {
		color: color-mix(in srgb, var(--text-2) 86%, transparent);
	}

	.maximized .windows-titlebar {
		padding-top: 4px;
	}

	.windows-chrome :global(.chrome-header) {
		padding-top: 8px;
	}

	.windows-titlebar__selector :global(.chrome-project-selector) {
		min-width: 0;
	}
</style>
