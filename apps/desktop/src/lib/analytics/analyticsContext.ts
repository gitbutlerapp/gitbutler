type StateValue = string | number | boolean | undefined;

interface StateData {
	[key: string]: StateValue;
}

/**
 * Stuff that gets added to posthog events.
 */
export class AnalyticsContext {
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
