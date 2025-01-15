import { shallowCompare } from '$lib/shallowCompare';

export interface Interest {
	_subscribe: () => () => void;
}

class Subscription<Arguments> {
	private counter = 0;
	private lastCalled = 0;
	private interval?: ReturnType<typeof setInterval>;

	constructor(
		readonly args: Arguments,
		private readonly upsert: () => void,
		private readonly frequency: number
	) {}

	createInterest(): Interest {
		return {
			_subscribe: () => {
				// Fetch data immediately on first subscription
				if (this.counter === 0) {
					if (Date.now() - this.lastCalled > this.frequency) {
						this.lastCalled = Date.now();
						this.upsert();
					}

					this.interval = setInterval(() => {
						this.lastCalled = Date.now();
						this.upsert();
					}, this.frequency);
				}

				++this.counter;

				let unsubscribed = false;

				// Unsubscribe function
				return () => {
					if (unsubscribed) {
						return;
					}
					unsubscribed = true;

					--this.counter;
					if (this.counter <= 0) {
						clearInterval(this.interval);
					}
				};
			}
		};
	}

	refetch(): void {
		this.upsert();
	}
}

export class InterestStore<Arguments> {
	private readonly subscriptions: Subscription<Arguments>[] = [];

	constructor(private readonly frequency: number) {}

	findOrCreateSubscribable(args: Arguments, upsert: () => void): Subscription<Arguments> {
		let subscription = this.subscriptions.find((subscription) =>
			shallowCompare(subscription.args, args)
		);
		if (!subscription) {
			subscription = new Subscription(args, upsert, this.frequency);
			this.subscriptions.push(subscription);
		}

		return subscription;
	}

	invalidate(args: Arguments): void {
		const subscription = this.subscriptions.find((subscription) =>
			shallowCompare(subscription.args, args)
		);
		if (subscription) {
			subscription.refetch();
		}
	}
}
