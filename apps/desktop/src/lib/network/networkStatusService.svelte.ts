import { InjectionToken } from '@gitbutler/core/context';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import type { Reactive } from '@gitbutler/shared/storeUtils';

export const NETWORK_STATUS_SERVICE = new InjectionToken<NetworkStatusService>(
	'NetworkStatusService'
);

/**
 * Service that monitors network online/offline status and provides reactive access to it.
 */
export class NetworkStatusService {
	private _online = $state<boolean>(typeof navigator !== 'undefined' ? navigator.onLine : true);

	constructor() {
		if (typeof window !== 'undefined') {
			window.addEventListener('online', this.handleOnline);
			window.addEventListener('offline', this.handleOffline);

			// Log initial status
			console.log(`Network status: ${this._online ? 'ONLINE' : 'OFFLINE'}`);
		}
	}

	private handleOnline = () => {
		this._online = true;
		console.log('Network status: ONLINE');
	};

	private handleOffline = () => {
		this._online = false;
		console.log('Network status: OFFLINE');
	};

	/**
	 * Reactive getter for online status.
	 * Returns true when the browser has network connectivity, false otherwise.
	 */
	get online(): Reactive<boolean> {
		return reactive(() => this._online);
	}

	/**
	 * Clean up event listeners
	 */
	destroy() {
		if (typeof window !== 'undefined') {
			window.removeEventListener('online', this.handleOnline);
			window.removeEventListener('offline', this.handleOffline);
		}
	}
}
