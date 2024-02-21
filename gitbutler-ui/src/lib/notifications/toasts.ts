import { writable, type Writable } from 'svelte/store';

export type ToastStyle = 'neutral' | 'error' | 'pop' | 'warn';

export interface ToastMessage {
	id?: number;
	message: string;
	title?: string;
	style?: ToastStyle;
}

export const toastStore: Writable<ToastMessage[]> = writable([]);

let idCounter = 0;

export function showToast(message: ToastMessage) {
	message.message = message.message.replace(/^ */gm, '');
	toastStore.update((items) => [...items, { id: idCounter++, ...message }]);
}

export function dismissToast(messageId: number | undefined) {
	toastStore.update((items) => items.filter((m) => m.id != messageId));
}
