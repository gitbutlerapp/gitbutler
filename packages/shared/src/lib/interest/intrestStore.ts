import { shallowCompare } from '$lib/shallowCompare';

export interface Interest {
	_subscribe: () => () => void;
}

interface Subscription<Arguments> {
	args: Arguments;
	counter: number;
	interval?: ReturnType<typeof setInterval>;
	lastCalled: number;
}

export class InterestStore<Arguments> {
	private readonly subscriptions: Subscription<Arguments>[] = [];

	constructor(private readonly frequency: number) {}

	createInterest(args: Arguments, callback: () => void): Interest {
		return {
			_subscribe: () => {
				let subscription = this.subscriptions.find((subscription) =>
					shallowCompare(subscription.args, args)
				);
				if (!subscription) {
					subscription = {
						args,
						counter: 0,
						lastCalled: 0,
						interval: undefined
					} as Subscription<Arguments>;
				}

				this.subscriptions.push(subscription);

				// Fetch data immediately on first subscription
				if (subscription.counter === 0) {
					if (Date.now() - subscription.lastCalled > this.frequency) {
						subscription.lastCalled = Date.now();
						callback();
					}

					subscription.interval = setInterval(() => {
						subscription.lastCalled = Date.now();
						callback();
					}, this.frequency);
				}

				++subscription.counter;

				let unsubscribed = false;

				// Unsubscribe function
				return () => {
					if (unsubscribed) {
						return;
					}
					unsubscribed = true;

					--subscription.counter;
					if (subscription.counter <= 0) {
						clearInterval(subscription.interval);
					}
				};
			}
		};
	}
}
