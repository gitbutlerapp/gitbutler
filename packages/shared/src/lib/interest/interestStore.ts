import { shallowCompare } from '$lib/shallowCompare';
import { asyncToSyncSignals } from '$lib/storeUtils';

type Callable =
	| (() => Promise<void>)
	| (() => Promise<() => void>)
	| (() => Promise<() => Promise<void>>)
	| (() => void)
	| (() => () => void)
	| (() => () => Promise<void>);

/**
 * A handle used to inform a subscription whether it should be polling/listening
 * or not.
 *
 * The interest is designed to be consumed by the `registerInterest` function
 * which will tie it's lifetime to that of a given component.
 */
export interface Interest {
	_subscribe: () => () => void;
}

/**
 * Subscriptions are internal data structures used as part of the interest
 * system. Inside one InterestStore, there should only ever be one instance
 * of a Subscription with a given set of `args` (determined with a shallow
 * compare).
 */
type Subscription<Arguments> = {
	args: Arguments;
	createInterest: () => Interest;
	refetch: () => Promise<void>;
};

class PollingSubscription<Arguments> implements Subscription<Arguments> {
	private counter = 0;
	private lastCalled = 0;
	private interval?: ReturnType<typeof setInterval>;
	private unsubscribe?: () => Promise<void> | void;

	constructor(
		readonly args: Arguments,
		private readonly upsert: Callable,
		private readonly frequency: number
	) {}

	createInterest(): Interest {
		return {
			_subscribe: asyncToSyncSignals(this.subscribe.bind(this))
		};
	}

	private async subscribe() {
		// Fetch data immediately on first subscription
		if (this.counter === 0) {
			// If there is no frequency, then we should always make the initial fetch
			if (Date.now() - this.lastCalled > this.frequency) {
				this.lastCalled = Date.now();
				this.unsubscribe = (await this.upsert()) || undefined;
			}

			this.interval = setInterval(async () => {
				this.lastCalled = Date.now();
				this.unsubscribe = (await this.upsert()) || undefined;
			}, this.frequency);
		}

		++this.counter;

		// Unsubscribe function
		return async () => {
			--this.counter;

			if (this.counter <= 0) {
				clearInterval(this.interval);
				await this.unsubscribe?.();
			}
		};
	}

	async refetch(): Promise<void> {
		await this.upsert();
	}
}

class ListeningSubscription<Arguments> implements Subscription<Arguments> {
	private counter = 0;
	private unsubscribe?: () => Promise<void> | void;

	constructor(
		readonly args: Arguments,
		private readonly upsert: Callable
	) {}

	createInterest(): Interest {
		return {
			_subscribe: asyncToSyncSignals(this.subscribe.bind(this))
		};
	}

	private async subscribe() {
		// Fetch data immediately on first subscription
		if (this.counter === 0) {
			this.unsubscribe = (await this.upsert()) || undefined;
		}

		++this.counter;

		// Unsubscribe function
		return async () => {
			--this.counter;

			if (this.counter <= 0) {
				await this.unsubscribe?.();
			}
		};
	}

	async refetch(): Promise<void> {
		// Refetch does not make sense for listening subscriptions
		throw new Error('Refetch can not be called on ListeningSubscription');
	}
}

/**
 * The interest store is responsible for managing the subscriptions produced by
 * given service. There will only ever be one interest for each set of `args`
 * provided. This allows us to avoid making duplicate or supuflous requests.
 *
 * If it's constructed with a `frequency` the start notifer will get called first
 * when the subscription is first registered, and then many subsequent times,
 * until it is no longer registered.
 *
 * If it's constructed without a `frequency`, the start notifer will only get
 * called once before it's stop notifier is called.
 */
export class InterestStore<Arguments> {
	private readonly subscriptions: Subscription<Arguments>[] = [];

	constructor(private readonly frequency?: number) {}

	findOrCreateSubscribable(args: Arguments, upsert: Callable): Subscription<Arguments> {
		let subscription = this.subscriptions.find((subscription) =>
			shallowCompare(subscription.args, args)
		);
		if (!subscription) {
			if (this.frequency) {
				subscription = new PollingSubscription(args, upsert, this.frequency);
			} else {
				subscription = new ListeningSubscription(args, upsert);
			}
			this.subscriptions.push(subscription);
		}

		return subscription;
	}

	async invalidate(args: Arguments): Promise<void> {
		const subscription = this.subscriptions.find((subscription) =>
			shallowCompare(subscription.args, args)
		);
		if (subscription) {
			await subscription.refetch();
		}
	}
}
