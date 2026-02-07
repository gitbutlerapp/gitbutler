import { InjectionToken } from "@gitbutler/core/context";
import type { SharedStackPayload } from "$lib/irc/sharedStack";

export type ReceivedStack = {
	id: string;
	receivedAt: number;
	sender: string;
	payload: SharedStackPayload;
};

export const RECEIVED_STACKS_STORE = new InjectionToken<ReceivedStacksStore>("ReceivedStacksStore");

export class ReceivedStacksStore {
	stacks = $state<ReceivedStack[]>([]);

	add(payload: SharedStackPayload, sender: string, timestamp: number): void {
		const id = `${sender}-${timestamp}-${Math.random().toString(36).slice(2)}`;
		this.stacks = [...this.stacks, { id, receivedAt: timestamp, sender, payload }];
	}

	remove(id: string): void {
		this.stacks = this.stacks.filter((s) => s.id !== id);
	}

	clear(): void {
		this.stacks = [];
	}
}
