/**
 * Time-related utility functions
 */

/**
 * Checks if the current time is within business hours (9am-5pm, Monday-Friday)
 * @param now Optional date to check (defaults to current time)
 * @returns boolean indicating if it's currently business hours
 */
export function isBusinessHours(now: Date = new Date()): boolean {
	// Get day of week (0 = Sunday, 1 = Monday, ..., 6 = Saturday)
	const dayOfWeek = now.getDay();

	// Business days are Monday (1) through Friday (5)
	const isBusinessDay = dayOfWeek >= 1 && dayOfWeek <= 5;

	if (!isBusinessDay) return false;

	// Business hours are 9am to 5pm
	return now.getHours() >= 9 && now.getHours() < 17;
}
