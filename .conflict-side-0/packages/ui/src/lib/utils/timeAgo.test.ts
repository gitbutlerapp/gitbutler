import { getTimeAgo } from '$lib/utils/timeAgo';
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
