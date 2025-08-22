export interface MenuInstance {
	id: string;
	element: HTMLElement;
	parentMenuId?: string;
	close: () => void;
}

class MenuManager {
	private menus = new Map<string, MenuInstance>();
	private clickHandler = this.handleGlobalClick.bind(this);
	private isInitialized = false;

	private initGlobalListener() {
		if (!this.isInitialized) {
			document.addEventListener('pointerdown', this.clickHandler, true);
			document.addEventListener('contextmenu', this.clickHandler, true);
			this.isInitialized = true;
		}
	}

	private cleanupGlobalListener() {
		if (this.isInitialized && this.menus.size === 0) {
			document.removeEventListener('pointerdown', this.clickHandler, true);
			document.removeEventListener('contextmenu', this.clickHandler, true);
			this.isInitialized = false;
		}
	}

	private handleGlobalClick(event: MouseEvent) {
		const target = event.target as HTMLElement;

		// Find if the click is inside any menu
		let clickedMenu: MenuInstance | null = null;
		for (const menu of this.menus.values()) {
			if (menu.element.contains(target)) {
				clickedMenu = menu;
				break;
			}
		}

		// If no menu was clicked, close all menus
		if (!clickedMenu) {
			this.closeAll();
			return;
		}

		// If clicked inside a menu, close any sibling menus (same parent)
		// but keep parent menus open
		for (const menu of this.menus.values()) {
			if (menu.id !== clickedMenu.id && menu.parentMenuId === clickedMenu.parentMenuId) {
				this.closeMenu(menu.id);
			}
		}
	}

	register(menu: MenuInstance) {
		this.menus.set(menu.id, menu);
		this.initGlobalListener();
	}

	unregister(menuId: string) {
		this.menus.delete(menuId);
		this.cleanupGlobalListener();
	}

	closeMenu(menuId: string) {
		const menu = this.menus.get(menuId);
		if (menu) {
			// First close all child menus
			this.closeChildren(menuId);
			// Then close this menu
			menu.close();
			this.menus.delete(menuId);
			this.cleanupGlobalListener();
		}
	}

	closeChildren(parentMenuId: string) {
		const childMenus = Array.from(this.menus.values()).filter(
			(menu) => menu.parentMenuId === parentMenuId
		);
		for (const child of childMenus) {
			this.closeMenu(child.id);
		}
	}

	closeAll() {
		const menus = Array.from(this.menus.values());
		for (const menu of menus) {
			menu.close();
		}
		this.menus.clear();
		this.cleanupGlobalListener();
	}

	isMenuOpen(menuId: string): boolean {
		return this.menus.has(menuId);
	}

	getMenu(menuId: string): MenuInstance | undefined {
		return this.menus.get(menuId);
	}
}

// Global singleton instance
export const menuManager = new MenuManager();
