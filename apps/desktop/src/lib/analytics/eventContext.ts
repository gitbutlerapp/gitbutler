import { InjectionToken } from '@gitbutler/core/context';

type StateValue = string | number | boolean | undefined;

interface StateData {
	[key: string]: StateValue;
}

export const EVENT_CONTEXT = new InjectionToken<EventContext>('EventContext');

/**
 * Stuff that gets added to posthog events.
 */
export class EventContext {
	private state: StateData = {};

	set(key: string, value: StateValue): void {
		this.state[key] = value;
	}

	get(key: string): StateValue | undefined {
		return this.state[key];
	}

	update(updates: StateData): void {
		Object.assign(this.state, updates);
	}

	getAll(): StateData {
		return { ...this.state };
	}
}
