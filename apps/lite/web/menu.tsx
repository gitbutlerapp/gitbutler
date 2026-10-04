/**
 * The browser's stand-in for Electron's native menus: the harness panel's popup
 * (`harness/deepseek/client.js`), drawing the items `native-menu.ts` sends Electron at the same
 * place and answering with the chosen item's id. Keep the two in step.
 */
import type { ShowNativeMenuParams } from "#electron/ipc.ts";
import { type FC, useSyncExternalStore } from "react";
import { createRoot } from "react-dom/client";
import { NativeMenu, type OpenMenu } from "./NativeMenu.tsx";
import { withOpenInGitButler } from "./open-in-gitbutler.ts";

let openMenu: OpenMenu | null = null;
const listeners = new Set<() => void>();
const setMenu = (menu: OpenMenu | null) => {
	openMenu = menu;
	for (const listener of listeners) listener();
};
const subscribe = (listener: () => void) => {
	listeners.add(listener);
	return () => listeners.delete(listener);
};

/** Show `params`' menu, resolving with the chosen item's id, or null once it's dismissed. */
export const showMenu = (params: ShowNativeMenuParams): Promise<string | null> =>
	new Promise((resolve) => {
		// A second menu supersedes an open one: resolve the old as cancelled.
		openMenu?.close();
		setMenu({
			...params,
			items: withOpenInGitButler(params.items, params.context),
			resolve,
			close: () => {
				setMenu(null);
				resolve(null);
			},
		});
	});

const MenuHost: FC = () => {
	const menu = useSyncExternalStore(subscribe, () => openMenu);
	return menu === null ? null : <NativeMenu menu={menu} />;
};

/** Mount the host the menus draw into, beside the app's own root. */
export const mountMenuHost = () => {
	const container = document.createElement("div");
	document.body.append(container);
	createRoot(container).render(<MenuHost />);
};
