import { shallowCompare } from '$lib/shallowCompare';

export interface Interest {
	_subscribe: () => () => void;
}

interface Subscription<Arguments> {
	args: Arguments;
	counter: number;
	interval: ReturnType<typeof setInterval>;
	lastCalled: number;
}

export class InterestStore<Arguments> {
	private readonly subscriptions: Subscription<Arguments>[] = [];

	constructor(private readonly frequency: number) {}

	createInterest(args: Arguments, callback: (args: Arguments) => void): Interest {
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
						interval: setInterval(() => {
							callback(args);
						}, this.frequency)
					} as Subscription<Arguments>;
				}

				this.subscriptions.push(subscription);

				// Fetch data immediately on first subscription
				if (subscription.counter === 0) {
					callback(args);
				}

				++subscription.counter;

				// Unsubscribe function
				return () => {
					--subscription.counter;
					if (subscription.counter <= 0) {
						clearInterval(subscription.interval);
					}
				};
			}
		};
	}
}
