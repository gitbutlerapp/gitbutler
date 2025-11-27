import { getTimeAgo, getAbsoluteTimestamp } from '$lib/utils/timeAgo';
import dayjs from 'dayjs';
import { describe, it, expect } from 'vitest';

describe.concurrent('timeAgo without suffix', () => {
	it('should format 30s ago correctly', () => {
		const date = dayjs().subtract(30, 'second').toDate();
		expect(getTimeAgo(date, false)).toBe('a few sec');
	});

	it('should format 3 hours ago correctly', () => {
		const date = dayjs().subtract(3, 'hour').toDate();
		expect(getTimeAgo(date, false)).toBe('3 hours');
	});

	it('should format 1 day ago correctly', () => {
		const date = dayjs().subtract(1, 'day').toDate();
		expect(getTimeAgo(date, false)).toBe('a day');
	});
});

describe.concurrent('timeAgo', () => {
	it('should format 9s ago correctly', () => {
		const date = dayjs().subtract(9, 'second').toDate();
		expect(getTimeAgo(date)).toBe('just now');
	});

	it('should format 30s ago correctly', () => {
		const date = dayjs().subtract(30, 'second').toDate();
		expect(getTimeAgo(date)).toBe('a few sec ago');
	});

	it('should format 3 mins ago correctly', () => {
		const date = dayjs().subtract(3, 'minute').toDate();
		expect(getTimeAgo(date)).toBe('3 min ago');
	});

	it('should format hours ago singular correctly', () => {
		const date = dayjs().subtract(1, 'hour').toDate();
		expect(getTimeAgo(date)).toBe('an hour ago');
	});

	it('should format 3 hours ago correctly', () => {
		const date = dayjs().subtract(3, 'hour').toDate();
		expect(getTimeAgo(date)).toBe('3 hours ago');
	});

	it('should format day ago singular correctly', () => {
		const date = dayjs().subtract(1, 'day').toDate();
		expect(getTimeAgo(date)).toBe('a day ago');
	});

	it('should format 2 days ago correctly', () => {
		const date = dayjs().subtract(2, 'day').toDate();
		expect(getTimeAgo(date)).toBe('2 days ago');
	});

	it('should format 1 month ago correctly', () => {
		const date = dayjs().subtract(1, 'month').toDate();
		expect(getTimeAgo(date)).toBe('a mo ago');
	});

	it('should format 2 months ago correctly', () => {
		const date = dayjs().subtract(2, 'month').toDate();
		expect(getTimeAgo(date)).toBe('2 mo ago');
	});

	it('should format 1 year ago correctly', () => {
		const date = dayjs().subtract(1, 'year').toDate();
		expect(getTimeAgo(date)).toBe('a yr ago');
	});

	it('should format 2 years ago correctly', () => {
		const date = dayjs().subtract(2, 'year').toDate();
		expect(getTimeAgo(date)).toBe('2 yr ago');
	});
});

describe.concurrent('getAbsoluteTimestamp', () => {
	it('should format a date correctly', () => {
		const date = new Date('2024-01-15T15:45:30.000Z');
		const result = getAbsoluteTimestamp(date, 'en-US');
		expect(result).toMatch(/January 15, 2024 at \d{1,2}:\d{2} [AP]M/);
	});

	it('should format a timestamp correctly', () => {
		const timestamp = new Date('2024-12-25T09:30:00.000Z').getTime();
		const result = getAbsoluteTimestamp(timestamp, 'en-US');
		expect(result).toMatch(/December 25, 2024 at \d{1,2}:\d{2} [AP]M/);
	});

	it('should handle different times of day', () => {
		const morningDate = new Date('2024-03-10T08:15:00.000Z');
		const eveningDate = new Date('2024-03-10T20:30:00.000Z');

		const morningResult = getAbsoluteTimestamp(morningDate, 'en-US');
		const eveningResult = getAbsoluteTimestamp(eveningDate, 'en-US');

		expect(morningResult).toMatch(/March 10, 2024 at \d{1,2}:\d{2} [AP]M/);
		expect(eveningResult).toMatch(/March 10, 2024 at \d{1,2}:\d{2} [AP]M/);
	});
});
