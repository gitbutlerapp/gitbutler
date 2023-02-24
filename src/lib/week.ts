import { startOfWeek, endOfWeek, addWeeks, subWeeks, addDays } from 'date-fns';

export type Week = {
	start: Date;
	end: Date;
};
export namespace Week {
	export const from = (date: Date): Week => {
		return {
			start: startOfWeek(date, { weekStartsOn: 1 }),
			end: endOfWeek(date)
		};
	};
	export const next = (week: Week): Week => {
		return { start: addWeeks(week.start, 1), end: addWeeks(week.end, 1) };
	};
	export const previous = (week: Week): Week => {
		return { start: subWeeks(week.start, 1), end: subWeeks(week.end, 1) };
	};
	export const nThDay = (week: Week, n: number): Date => {
		return addDays(week.start, n);
	};
}
